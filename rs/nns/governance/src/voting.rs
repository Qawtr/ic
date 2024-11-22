use crate::{
    governance::Governance,
    neuron_store::NeuronStore,
    pb::v1::{Ballot, Topic, Topic::NeuronManagement, Vote},
    storage::with_voting_state_machines_mut,
};
use ic_nervous_system_long_message::{
    break_message_if_over_instructions, is_message_over_threshold,
};
use ic_nns_common::pb::v1::{NeuronId, ProposalId};
use ic_stable_structures::{storable::Bound, StableBTreeMap, Storable};
use prost::Message;
use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, HashMap},
};

const BILLION: u64 = 1_000_000_000;

const SOFT_VOTING_INSTRUCTIONS_LIMIT: u64 = 2 * BILLION;
const HARD_VOTING_INSTRUCTIONS_LIMIT: u64 = 750 * BILLION;

impl Governance {
    pub async fn cast_vote_and_cascade_follow(
        &mut self,
        proposal_id: ProposalId,
        voting_neuron_id: NeuronId,
        vote_of_neuron: Vote,
        topic: Topic,
    ) {
        // First we cast the ballot.
        with_voting_state_machines_mut(|voting_state_machines| {
            let ballots = &mut self
                .heap_data
                .proposals
                .get_mut(&proposal_id.id)
                .unwrap()
                .ballots;
            voting_state_machines.with_machine(proposal_id, topic, |machine| {
                machine.cast_vote(ballots, voting_neuron_id, vote_of_neuron)
            });
        });

        // Next we create a loop that re-aquires context and continues processing until done.
        // Re-acquiring context is necessary because the state machines are stored in thread local
        // but the ballots can change between messages (even though this is all in one call context)
        // as the calculation is divided by a canister self-call.
        let mut is_done = false;

        fn over_soft_message_limit() -> bool {
            is_message_over_threshold(SOFT_VOTING_INSTRUCTIONS_LIMIT)
        }

        while !is_done {
            // Now we process until either A we are done or B, we are over a limit and need to
            // make a self-call
            with_voting_state_machines_mut(|voting_state_machines| {
                // Reacquire context
                let neuron_store = &mut self.neuron_store;
                let ballots = &mut self
                    .heap_data
                    .proposals
                    .get_mut(&proposal_id.id)
                    .unwrap()
                    .ballots;

                voting_state_machines.with_machine(proposal_id, topic, |machine| {
                    is_done = machine.is_done();
                    while !is_done {
                        machine.continue_processing(neuron_store, ballots);
                        is_done = machine.is_done();

                        if over_soft_message_limit() {
                            break;
                        }
                    }
                });
            });
            // We send a no-op message to self to break up the call context into more messages
            break_message_if_over_instructions(
                SOFT_VOTING_INSTRUCTIONS_LIMIT,
                Some(HARD_VOTING_INSTRUCTIONS_LIMIT),
            )
            .await;
        }
    }
}

pub(crate) struct VotingStateMachines<Memory>
where
    Memory: ic_stable_structures::Memory,
{
    // Up to one machine per proposal, to avoid having to do unnecessary checks for followers that
    // might follow.  This allows the state machines to be used across multiple messages
    // without duplicating state and memory usage.
    machines: StableBTreeMap<ProposalId, crate::pb::v1::ProposalVotingStateMachine, Memory>,
}

impl<Memory: ic_stable_structures::Memory> VotingStateMachines<Memory> {
    pub(crate) fn new(memory: Memory) -> Self {
        Self {
            machines: StableBTreeMap::init(memory),
        }
    }

    /// Perform a callback with a given voting machine.  If the machine is finished, it is removed
    /// after the callback.
    fn with_machine<R>(
        &mut self,
        proposal_id: ProposalId,
        topic: Topic,
        callback: impl FnOnce(&mut ProposalVotingStateMachine) -> R,
    ) -> R {
        // We use remove here because we delete machines if they're done.
        // This reduces stable memory calls in the case where the machine is completed,
        // as we do not need to get it and then remove it later.
        let mut machine = self
            .machines
            .remove(&proposal_id)
            // This unwrap should be safe because we only write valid machines below.
            .map(|proto| ProposalVotingStateMachine::try_from(proto).unwrap())
            .unwrap_or(ProposalVotingStateMachine::try_new(proposal_id, topic).unwrap());

        let result = callback(&mut machine);

        // Save the machine again if it's not finished.
        if !machine.is_done() {
            self.machines.insert(
                proposal_id,
                crate::pb::v1::ProposalVotingStateMachine::from(machine),
            );
        }
        result
    }
}

#[derive(Debug, PartialEq, Default)]
struct ProposalVotingStateMachine {
    // The proposal ID that is being voted on.
    proposal_id: ProposalId,
    // The topic of the proposal.
    topic: Topic,
    // Votes that have been cast before checking followees
    neurons_to_check_followers: BTreeSet<NeuronId>,
    // followers to process
    followers_to_check: BTreeSet<NeuronId>,
    // votes that need to be recorded in each neuron's recent_ballots
    recent_neuron_ballots_to_record: BTreeMap<NeuronId, Vote>,
}

impl From<ProposalVotingStateMachine> for crate::pb::v1::ProposalVotingStateMachine {
    fn from(value: ProposalVotingStateMachine) -> Self {
        Self {
            proposal_id: Some(value.proposal_id),
            topic: value.topic as i32,
            neurons_to_check_followers: value.neurons_to_check_followers.into_iter().collect(),
            followers_to_check: value.followers_to_check.into_iter().collect(),
            recent_neuron_ballots_to_record: value
                .recent_neuron_ballots_to_record
                .into_iter()
                .map(|(n, v)| (n.id, v as i32))
                .collect(),
        }
    }
}

impl TryFrom<crate::pb::v1::ProposalVotingStateMachine> for ProposalVotingStateMachine {
    type Error = String;

    fn try_from(value: crate::pb::v1::ProposalVotingStateMachine) -> Result<Self, Self::Error> {
        Ok(Self {
            proposal_id: value.proposal_id.ok_or("Proposal ID must be specified")?,
            topic: Topic::try_from(value.topic).map_err(|e| e.to_string())?,
            neurons_to_check_followers: value.neurons_to_check_followers.into_iter().collect(),
            followers_to_check: value.followers_to_check.into_iter().collect(),
            recent_neuron_ballots_to_record: value
                .recent_neuron_ballots_to_record
                .into_iter()
                .map(|(n, v)| {
                    let neuron_id = NeuronId::from_u64(n);
                    let vote = Vote::try_from(v).map_err(|e| e.to_string())?; // Propagate the error directly
                    Ok((neuron_id, vote))
                })
                .collect::<Result<_, Self::Error>>()?,
        })
    }
}

impl Storable for crate::pb::v1::ProposalVotingStateMachine {
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        Cow::from(self.encode_to_vec())
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        Self::decode(&bytes[..])
            // Convert from Result to Self. (Unfortunately, it seems that
            // panic is unavoid able in the case of Err.)
            .expect("Unable to deserialize ProposalVotingStateMachine.")
    }

    const BOUND: Bound = Bound::Unbounded;
}

impl ProposalVotingStateMachine {
    fn try_new(proposal_id: ProposalId, topic: Topic) -> Result<Self, String> {
        if topic == Topic::Unspecified {
            return Err("Topic must be specified".to_string());
        }

        Ok(Self {
            proposal_id,
            topic,
            ..Default::default()
        })
    }

    fn is_done(&self) -> bool {
        self.neurons_to_check_followers.is_empty()
            && self.followers_to_check.is_empty()
            && self.recent_neuron_ballots_to_record.is_empty()
    }

    fn add_followers_to_check(
        &mut self,
        neuron_store: &NeuronStore,
        voting_neuron: NeuronId,
        topic: Topic,
    ) {
        self.followers_to_check
            .extend(neuron_store.get_followers_by_followee_and_topic(voting_neuron, topic));
        if ![Topic::Governance, Topic::SnsAndCommunityFund].contains(&topic) {
            // Insert followers from 'Unspecified' (default followers)
            self.followers_to_check.extend(
                neuron_store.get_followers_by_followee_and_topic(voting_neuron, Topic::Unspecified),
            );
        }
    }

    fn cast_vote(&mut self, ballots: &mut HashMap<u64, Ballot>, neuron_id: NeuronId, vote: Vote) {
        // There is no action to take with unspecfiied votes, so we early return.  It is
        // a legitimate argument in the context of continue_processing, but it simply means
        // that no vote is cast, and therefore there is no followup work to do.
        // This condition is also important to ensure that the state machine always terminates
        // even if an Unspecified vote is somehow cast manually.
        if vote == Vote::Unspecified {
            return;
        }

        if let Some(ballot) = ballots.get_mut(&neuron_id.id) {
            // The following conditional is CRITICAL, as it prevents a neuron's vote from
            // being overwritten by a later vote. This is important because otherwse
            // a cyclic voting graph is possible, which could result in never finishing voting.
            if ballot.vote == Vote::Unspecified as i32 {
                // Cast vote in ballot
                ballot.vote = vote as i32;
                // record the votes that have been cast, to log
                self.recent_neuron_ballots_to_record.insert(neuron_id, vote);

                // Do not check followers for NeuronManagement topic
                if self.topic != NeuronManagement {
                    self.neurons_to_check_followers.insert(neuron_id);
                }
            }
        }
    }

    fn continue_processing(
        &mut self,
        neuron_store: &mut NeuronStore,
        ballots: &mut HashMap<u64, Ballot>,
    ) {
        while let Some(neuron_id) = self.neurons_to_check_followers.pop_first() {
            self.add_followers_to_check(neuron_store, neuron_id, self.topic);
        }

        // Memory optimization, will not cause tests to fail if removed
        retain_neurons_with_castable_ballots(&mut self.followers_to_check, ballots);

        while let Some(follower) = self.followers_to_check.pop_first() {
            let vote = match neuron_store.neuron_would_follow_ballots(follower, self.topic, ballots)
            {
                Ok(vote) => vote,
                Err(e) => {
                    // This is a bad inconsistency, but there is
                    // nothing that can be done about it at this
                    // place.  We somehow have followers recorded that don't exist.
                    eprintln!("error in cast_vote_and_cascade_follow when gathering induction votes: {:?}", e);
                    Vote::Unspecified
                }
            };
            // Casting vote immediately might affect other follower votes, which makes
            // voting resolution take fewer iterations.
            // Vote::Unspecified is ignored by cast_vote.
            self.cast_vote(ballots, follower, vote);
        }

        while let Some((neuron_id, vote)) = self.recent_neuron_ballots_to_record.pop_first() {
            match neuron_store.register_recent_neuron_ballot(
                neuron_id,
                self.topic,
                self.proposal_id,
                vote,
            ) {
                Ok(_) => {}
                Err(e) => {
                    // This is a bad inconsistency, but there is
                    // nothing that can be done about it at this
                    // place.  We somehow have followers recorded that don't exist.
                    eprintln!("error in cast_vote_and_cascade_follow when gathering induction votes: {:?}", e);
                }
            };
        }
    }
}

// Retain only neurons that have a ballot that can still be cast.  This excludes
// neurons with no ballots or ballots that have already been cast.
fn retain_neurons_with_castable_ballots(
    followers: &mut BTreeSet<NeuronId>,
    ballots: &HashMap<u64, Ballot>,
) {
    followers.retain(|f| {
        ballots
            .get(&f.id)
            // Only retain neurons with unspecified ballots
            .map(|b| b.vote == Vote::Unspecified as i32)
            // Neurons without ballots are also dropped
            .unwrap_or_default()
    });
}

#[cfg(test)]
mod test {

    use crate::{
        governance::MIN_DISSOLVE_DELAY_FOR_VOTE_ELIGIBILITY_SECONDS,
        neuron::{DissolveStateAndAge, Neuron, NeuronBuilder},
        neuron_store::NeuronStore,
        pb::v1::{neuron::Followees, Ballot, Topic, Vote},
        voting::ProposalVotingStateMachine,
    };
    use ic_base_types::PrincipalId;
    use ic_nns_common::pb::v1::{NeuronId, ProposalId};
    use icp_ledger::Subaccount;
    use maplit::{btreemap, hashmap};
    use std::collections::{BTreeMap, BTreeSet, HashMap};

    fn make_neuron(
        id: u64,
        cached_neuron_stake_e8s: u64,
        followees: HashMap<i32, Followees>,
    ) -> Neuron {
        let mut account = vec![0; 32];
        for (destination, data) in account.iter_mut().zip(id.to_le_bytes().iter().cycle()) {
            *destination = *data;
        }
        let subaccount = Subaccount::try_from(account.as_slice()).unwrap();

        let now = 123_456_789;
        let dissolve_state_and_age = DissolveStateAndAge::NotDissolving {
            dissolve_delay_seconds: MIN_DISSOLVE_DELAY_FOR_VOTE_ELIGIBILITY_SECONDS,
            aging_since_timestamp_seconds: now - MIN_DISSOLVE_DELAY_FOR_VOTE_ELIGIBILITY_SECONDS,
        };

        NeuronBuilder::new(
            NeuronId { id },
            subaccount,
            PrincipalId::new_user_test_id(id),
            dissolve_state_and_age,
            now,
        )
        .with_followees(followees)
        .with_cached_neuron_stake_e8s(cached_neuron_stake_e8s)
        .build()
    }

    fn add_neuron_with_ballot(
        neuron_store: &mut NeuronStore,
        ballots: &mut HashMap<u64, Ballot>,
        neuron: Neuron,
    ) {
        let cached_stake = neuron.cached_neuron_stake_e8s;
        let id = neuron.id().id;
        neuron_store
            .add_neuron(neuron)
            .expect("Couldn't add neuron");
        ballots.insert(
            id,
            Ballot {
                vote: Vote::Unspecified as i32,
                voting_power: cached_stake,
            },
        );
    }

    #[test]
    fn test_invalid_topic() {
        let err = ProposalVotingStateMachine::try_new(ProposalId { id: 0 }, Topic::Unspecified)
            .unwrap_err();

        assert_eq!(err, "Topic must be specified");
    }

    #[test]
    fn test_is_done() {
        let mut state_machine = ProposalVotingStateMachine {
            proposal_id: ProposalId { id: 0 },
            topic: Topic::Governance,
            neurons_to_check_followers: BTreeSet::new(),
            followers_to_check: BTreeSet::new(),
            recent_neuron_ballots_to_record: BTreeMap::new(),
        };

        assert!(state_machine.is_done());

        state_machine
            .neurons_to_check_followers
            .insert(NeuronId { id: 0 });
        assert!(!state_machine.is_done());
        state_machine.neurons_to_check_followers.clear();

        state_machine.followers_to_check.insert(NeuronId { id: 0 });
        assert!(!state_machine.is_done());
        state_machine.followers_to_check.clear();

        state_machine
            .recent_neuron_ballots_to_record
            .insert(NeuronId { id: 0 }, Vote::Yes);
        assert!(!state_machine.is_done());
        state_machine.recent_neuron_ballots_to_record.clear();
    }

    #[test]
    fn test_continue_processsing() {
        let mut state_machine =
            ProposalVotingStateMachine::try_new(ProposalId { id: 0 }, Topic::NetworkEconomics)
                .unwrap();

        let mut ballots = HashMap::new();
        let mut neuron_store = NeuronStore::new(btreemap! {});

        add_neuron_with_ballot(
            &mut neuron_store,
            &mut ballots,
            make_neuron(1, 101, hashmap! {}),
        );
        add_neuron_with_ballot(
            &mut neuron_store,
            &mut ballots,
            make_neuron(
                2,
                102,
                hashmap! {Topic::NetworkEconomics.into() => Followees {
                    followees: vec![NeuronId { id: 1 }],
                }},
            ),
        );

        state_machine.cast_vote(&mut ballots, NeuronId { id: 1 }, Vote::Yes);
        state_machine.continue_processing(&mut neuron_store, &mut ballots);

        assert_eq!(
            ballots,
            hashmap! {
            1 => Ballot { vote: Vote::Yes as i32, voting_power: 101 },
            2 => Ballot { vote: Vote::Yes as i32, voting_power: 102 }}
        );
        assert_eq!(
            neuron_store
                .with_neuron(&NeuronId { id: 1 }, |n| {
                    n.recent_ballots.first().unwrap().vote
                })
                .unwrap(),
            Vote::Yes as i32
        );
        assert_eq!(
            neuron_store
                .with_neuron(&NeuronId { id: 2 }, |n| {
                    n.recent_ballots.first().unwrap().vote
                })
                .unwrap(),
            Vote::Yes as i32
        );

        assert!(!state_machine.is_done());

        state_machine.continue_processing(&mut neuron_store, &mut ballots);

        assert_eq!(
            ballots,
            hashmap! {
            1 => Ballot { vote: Vote::Yes as i32, voting_power: 101 },
            2 => Ballot { vote: Vote::Yes as i32, voting_power: 102 }}
        );
        assert_eq!(
            neuron_store
                .with_neuron(&NeuronId { id: 1 }, |n| {
                    n.recent_ballots.first().unwrap().vote
                })
                .unwrap(),
            Vote::Yes as i32
        );
        assert_eq!(
            neuron_store
                .with_neuron(&NeuronId { id: 2 }, |n| {
                    n.recent_ballots.first().unwrap().vote
                })
                .unwrap(),
            Vote::Yes as i32
        );
        assert!(state_machine.is_done());
    }

    #[test]
    fn test_cyclic_following_will_terminate() {
        let mut state_machine =
            ProposalVotingStateMachine::try_new(ProposalId { id: 0 }, Topic::NetworkEconomics)
                .unwrap();

        let mut ballots = HashMap::new();
        let mut neuron_store = NeuronStore::new(btreemap! {});

        add_neuron_with_ballot(
            &mut neuron_store,
            &mut ballots,
            make_neuron(
                1,
                101,
                hashmap! {Topic::NetworkEconomics.into() => Followees {
                    followees: vec![NeuronId { id: 2 }],
                }},
            ),
        );
        add_neuron_with_ballot(
            &mut neuron_store,
            &mut ballots,
            make_neuron(
                2,
                102,
                hashmap! {Topic::NetworkEconomics.into() => Followees {
                    followees: vec![NeuronId { id: 1 }],
                }},
            ),
        );

        // We assert it is immediately done after casting an unspecified vote b/c there
        // is no work to do.
        state_machine.cast_vote(&mut ballots, NeuronId { id: 1 }, Vote::Unspecified);
        assert!(state_machine.is_done());

        // We assert it is done after checking both sets of followers
        state_machine.cast_vote(&mut ballots, NeuronId { id: 1 }, Vote::Yes);
        state_machine.continue_processing(&mut neuron_store, &mut ballots);
        state_machine.continue_processing(&mut neuron_store, &mut ballots);
        assert!(state_machine.is_done());
    }

    // TODO DO NOT MERGE How to test this?
    // What do I want to test?
    // 1. That the loop continues until done
    // 2. That the loop breaks if over the soft limit
    // 3. That it panics if over the hard limit
    // 4. That machine is cleaned up after it is done
    // 5. That we process votes before recording votes, and allow soft limit to push recording votes into async/timer
    // 6. That the timer will drain the queue of votes to record...
    // 7. That we can't lose data if we have to panic (and the votes get recorded in the timer)
    #[test]
    fn test_cast_vote_and_cascade_follow_always_finishes_processing_ballots() {}
    #[test]
    fn test_cast_vote_and_cascade_follow_breaks_at_soft_limit() {}
    #[test]
    fn test_cast_vote_and_cascade_follow_panics_if_over_hard_limit() {}
    #[test]
    fn test_voting_machines_cleans_up_machines_that_are_finished() {}
    #[test]
    fn test_cast_vote_and_cascade_follow_doesnt_record_recent_ballots_after_first_soft_limit() {}
    #[test]
    fn test_cast_vote_and_cascade_follow_processes_votes_before_recording_recent_ballots() {}
    #[test]
    fn test_voting_machine_timer_eventually_drains_queue() {}
    #[test]
    fn test_panic_does_not_lose_data() {}
}

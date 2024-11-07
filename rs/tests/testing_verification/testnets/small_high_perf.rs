// Set up a testnet containing:
//   one 1-node System and one 1-node Application subnets, single boundary node, and a p8s (with grafana) VM.
// All replica nodes use the following resources: 64 vCPUs, 480 GiB of RAM, and 2'000 GiB disk.
//
// You can setup this testnet with a lifetime of 180 mins by executing the following commands:
//
//   $ ./ci/tools/docker-run
//   $ ict testnet create small_high_perf --lifetime-mins=180 --output-dir=./small_high_perf -- --test_tmpdir=./small_high_perf
//
// The --output-dir=./small_high_perf will store the debug output of the test driver in the specified directory.
// The --test_tmpdir=./small_high_perf will store the remaining test output in the specified directory.
// This is useful to have access to in case you need to SSH into an IC node for example like:
//
//   $ ssh -i small_high_perf/_tmp/*/setup/ssh/authorized_priv_keys/admin admin@
//
// Note that you can get the  address of the IC node from the ict console output:
//
//   {
//     nodes: [
//       {
//         id: y4g5e-dpl4n-swwhv-la7ec-32ngk-w7f3f-pr5bt-kqw67-2lmfy-agipc-zae,
//         ipv6: 2a0b:21c0:4003:2:5034:46ff:fe3c:e76f
//       }
//     ],
//     subnet_id: 5hv4k-srndq-xgw53-r6ldt-wtv4x-6xvbj-6lvpf-sbu5n-sqied-63bgv-eqe,
//     subnet_type: application
//   },
//
// To get access to P8s and Grafana look for the following lines in the ict console output:
//
//     prometheus: Prometheus Web UI at http://prometheus.small_high_perf--1692597750709.testnet.farm.dfinity.systems,
//     grafana: Grafana at http://grafana.small_high_perf--1692597750709.testnet.farm.dfinity.systems,
//     progress_clock: IC Progress Clock at http://grafana.small_high_perf--1692597750709.testnet.farm.dfinity.systems/d/ic-progress-clock/ic-progress-clock?refresh=10su0026from=now-5mu0026to=now,
//
// Happy testing!

use anyhow::Result;

use ic_consensus_system_test_utils::rw_message::install_nns_with_customizations_and_check_progress;
use ic_registry_subnet_type::SubnetType;
use ic_system_test_driver::driver::{
    boundary_node::BoundaryNode,
    group::SystemTestGroup,
    ic::{
        AmountOfMemoryKiB,
        ImageSizeGiB,
        InternetComputer,
        NrOfVCPUs,
        Subnet,
        VmResources,
    },
    prometheus_vm::{
        HasPrometheus,
        PrometheusVm,
    },
    test_env::TestEnv,
    test_env_api::{
        await_boundary_node_healthy,
        HasTopologySnapshot,
        NnsCustomizations,
    },
};

const BOUNDARY_NODE_NAME: &str = "boundary-node-1";

fn main() -> Result<()> {
    SystemTestGroup::new()
        .with_setup(setup)
        .execute_from_args()?;
    Ok(())
}

pub fn setup(env: TestEnv) {
    PrometheusVm::default()
        .start(&env)
        .expect("Failed to start prometheus VM");
    InternetComputer::new()
        .with_default_vm_resources(VmResources {
            vcpus: Some(NrOfVCPUs::new(64)),
            memory_kibibytes: Some(AmountOfMemoryKiB::new(480 << 20)),
            boot_image_minimal_size_gibibytes: Some(ImageSizeGiB::new(2_000)),
        })
        .add_subnet(Subnet::new(SubnetType::System).add_nodes(1))
        .add_subnet(Subnet::new(SubnetType::Application).add_nodes(1))
        .setup_and_start(&env)
        .expect("Failed to setup IC under test");
    install_nns_with_customizations_and_check_progress(
        env.topology_snapshot(),
        NnsCustomizations::default(),
    );
    BoundaryNode::new(String::from(BOUNDARY_NODE_NAME))
        .allocate_vm(&env)
        .expect("Allocation of BoundaryNode failed.")
        .for_ic(&env, "")
        .use_real_certs_and_dns()
        .start(&env)
        .expect("failed to setup BoundaryNode VM");
    env.sync_with_prometheus();
    await_boundary_node_healthy(&env, BOUNDARY_NODE_NAME);
}

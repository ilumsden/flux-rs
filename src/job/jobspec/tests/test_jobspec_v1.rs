use serde_json::json;

use crate::duration::FluxDuration;
use crate::job::jobspec::jobspec_v1::{JobspecV1, PerResourceType};
use crate::job::jobspec::Jobspec;

// =========================================================================
// Helpers
// =========================================================================

/// Build a valid JSON representation of a V1 Jobspec.
/// This passes both the base `Jobspec` validation and the `JobspecV1` validation.
fn make_valid_v1_jobspec_json() -> serde_json::Value {
    json!({
        "version": 1,
        "resources": [{
            "type": "node",
            "count": 1
        }],
        "tasks": [{
            "command": ["sleep", "1"],
            "slot": "node",
            "count": {"per_slot": 1}
        }],
        "attributes": {
            "system": {
                "duration": 3600
            }
        }
    })
}

/// Build a base `Jobspec` from JSON.
fn make_base_jobspec(val: serde_json::Value) -> Jobspec {
    serde_json::from_value(val).expect("Failed to create base Jobspec for test")
}

// =========================================================================
// TryFrom<Jobspec> for JobspecV1 Validation
// =========================================================================

#[test]
fn try_from_valid_jobspec_succeeds() {
    let js = make_base_jobspec(make_valid_v1_jobspec_json());
    assert!(JobspecV1::try_from(js).is_ok());
}

#[test]
fn try_from_missing_system_attributes_returns_error() {
    let mut js_json = make_valid_v1_jobspec_json();
    // Remove "system" and replace with "user" to pass base Jobspec validation
    // but fail V1 validation.
    js_json["attributes"] = json!({"user": {}});
    let js = make_base_jobspec(js_json);
    assert!(JobspecV1::try_from(js).is_err());
}

#[test]
fn try_from_missing_duration_returns_error() {
    let mut js_json = make_valid_v1_jobspec_json();
    js_json["attributes"]["system"] = json!({"other_key": "val"});
    let js = make_base_jobspec(js_json);
    assert!(JobspecV1::try_from(js).is_err());
}

#[test]
fn try_from_non_numeric_duration_returns_error() {
    let mut js_json = make_valid_v1_jobspec_json();
    js_json["attributes"]["system"]["duration"] = json!("3600s"); // String, not number
    let js = make_base_jobspec(js_json);
    assert!(JobspecV1::try_from(js).is_err());
}

#[test]
fn try_from_invalid_task_count_returns_error() {
    let mut js_json = make_valid_v1_jobspec_json();
    // V1 only allows PerSlot or Total. PerResource should fail.
    js_json["tasks"][0]["count"] = json!({"per_resource": {"type": "node", "count": 1}});
    let js = make_base_jobspec(js_json);
    assert!(JobspecV1::try_from(js).is_err());
}

#[test]
fn try_from_invalid_resource_count_returns_error() {
    let mut js_json = make_valid_v1_jobspec_json();
    // V1 only allows Integer resource counts. Dict should fail.
    js_json["resources"][0]["count"] = json!({"min": 1, "max": 2});
    let js = make_base_jobspec(js_json);
    assert!(JobspecV1::try_from(js).is_err());
}

// =========================================================================
// PerResourceBuilder
// =========================================================================

#[test]
fn per_resource_ncores_only_succeeds() {
    let js = JobspecV1::per_resource(["sleep", "1"])
        .ncores(4)
        .build_jobspec()
        .unwrap();
    assert_eq!(js.tasks[0].command, vec!["sleep", "1"]);
}

#[test]
fn per_resource_nnodes_exclusive_succeeds() {
    let js = JobspecV1::per_resource(["sleep", "1"])
        .nnodes(2)
        .exclusive(true)
        .build_jobspec()
        .unwrap();
    assert_eq!(js.resources[0].resource_type, "node");
}

#[test]
fn per_resource_nnodes_ncores_succeeds() {
    let js = JobspecV1::per_resource(["sleep", "1"])
        .nnodes(2)
        .ncores(8) // 8 cores across 2 nodes = 4 cores per node
        .build_jobspec()
        .unwrap();
    assert_eq!(js.resources[0].resource_type, "node");
}

#[test]
fn per_resource_missing_both_nnodes_and_ncores_returns_error() {
    let result = JobspecV1::per_resource(["sleep", "1"]).build_jobspec();
    assert!(result.is_err());
}

#[test]
fn per_resource_exclusive_without_nnodes_returns_error() {
    let result = JobspecV1::per_resource(["sleep", "1"])
        .ncores(4)
        .exclusive(true)
        .build_jobspec();
    assert!(result.is_err());
}

#[test]
fn per_resource_gpus_without_nnodes_returns_error() {
    let result = JobspecV1::per_resource(["sleep", "1"])
        .ncores(4)
        .gpus_per_node(1)
        .build_jobspec();
    assert!(result.is_err());
}

#[test]
fn per_resource_type_without_count_returns_error() {
    let mut builder = JobspecV1::per_resource(["sleep", "1"]).ncores(4);
    builder.per_resource_type = Some(PerResourceType::Node);
    assert!(builder.build_jobspec().is_err());
}

#[test]
fn per_resource_count_without_type_returns_error() {
    let mut builder = JobspecV1::per_resource(["sleep", "1"]).ncores(4);
    builder.per_resource_count = Some(2);
    assert!(builder.build_jobspec().is_err());
}

#[test]
fn per_resource_ncores_less_than_nnodes_returns_error() {
    let result = JobspecV1::per_resource(["sleep", "1"])
        .nnodes(4)
        .ncores(2)
        .build_jobspec();
    assert!(result.is_err());
}

#[test]
fn per_resource_ncores_not_divisible_by_nnodes_returns_error() {
    let result = JobspecV1::per_resource(["sleep", "1"])
        .nnodes(3)
        .ncores(4)
        .build_jobspec();
    assert!(result.is_err());
}

#[test]
fn per_resource_delegates_to_base_options() {
    let js = JobspecV1::per_resource(["sleep", "1"])
        .ncores(1)
        .queue("debug")
        .duration(FluxDuration::Secs(120.0))
        .build_jobspec()
        .unwrap();
    assert_eq!(js.queue_as().unwrap(), "debug");
    assert!(matches!(js.duration_as().unwrap(), FluxDuration::Secs(s) if s == 120.0));
}

// =========================================================================
// FromCommandBuilder
// =========================================================================

#[test]
fn from_command_tasks_only_succeeds() {
    let js = JobspecV1::from_command(["sleep", "1"])
        .num_tasks(4)
        .build_jobspec()
        .unwrap();
    assert_eq!(js.tasks[0].command, vec!["sleep", "1"]);
}

#[test]
fn from_command_tasks_and_nodes_succeeds() {
    let js = JobspecV1::from_command(["sleep", "1"])
        .num_tasks(4)
        .num_nodes(2)
        .build_jobspec()
        .unwrap();
    assert_eq!(js.resources[0].resource_type, "node");
}

#[test]
fn from_command_nodes_greater_than_tasks_returns_error() {
    let result = JobspecV1::from_command(["sleep", "1"])
        .num_tasks(2)
        .num_nodes(4)
        .build_jobspec();
    assert!(result.is_err());
}

#[test]
fn from_command_exclusive_without_nodes_returns_error() {
    let result = JobspecV1::from_command(["sleep", "1"])
        .num_tasks(2)
        .exclusive(true)
        .build_jobspec();
    assert!(result.is_err());
}

// =========================================================================
// FromNestCommandBuilder
// =========================================================================

#[test]
fn from_nest_command_prepends_flux_broker() {
    let js = JobspecV1::from_nest_command(["my_app"])
        .num_slots(1)
        .build_jobspec()
        .unwrap();
    let cmd = &js.tasks[0].command;
    assert_eq!(cmd[0], "flux");
    assert_eq!(cmd[1], "broker");
    assert_eq!(cmd.last().unwrap(), "my_app");
}

#[test]
fn from_nest_command_with_inline_conf_adds_file() {
    // A conf string with a newline is treated as inline text content
    let js = JobspecV1::from_nest_command(["my_app"])
        .num_slots(1)
        .conf("{\n  \"key\": \"val\"\n}")
        .build_jobspec()
        .unwrap();

    let cmd = &js.tasks[0].command;
    assert!(cmd.contains(&"-c{{tmpdir}}/conf.json".to_string()));

    let files = js.get_attr("system.files").unwrap().as_object().unwrap();
    assert!(files.contains_key("conf.json"));
}

// =========================================================================
// FromBatchCommandBuilder
// =========================================================================

#[test]
fn from_batch_command_valid_script_succeeds() {
    let js = JobspecV1::from_batch_command("#!/bin/bash\nsleep 1", "my_job")
        .num_slots(1)
        .build_jobspec()
        .unwrap();

    let cmd = &js.tasks[0].command;
    assert_eq!(cmd.last().unwrap(), "{{tmpdir}}/script");

    let files = js.get_attr("system.files").unwrap().as_object().unwrap();
    assert!(files.contains_key("script"));

    // Job name should be set
    assert_eq!(
        js.get_attr_as::<String>("system.job.name").unwrap(),
        "my_job"
    );
}

#[test]
fn from_batch_command_missing_shebang_returns_error() {
    let result = JobspecV1::from_batch_command("sleep 1", "my_job")
        .num_slots(1)
        .build_jobspec();
    assert!(result.is_err());
}

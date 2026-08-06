use std::path::PathBuf;

use serde_json::Value;
use serde_json::json;

use crate::duration::FluxDuration;
use crate::job::jobspec::canonical_jobspec::{Jobspec, RawJobspec};
use crate::job::jobspec::fileref::Fileref;
use crate::job::jobspec::resource_vertex::ResourceVertex;
use crate::job::jobspec::task::Task;

// =========================================================================
// Helpers
// =========================================================================

fn make_resource_vertex(val: Value) -> ResourceVertex {
    serde_json::from_value(val).expect("Failed to create ResourceVertex for test")
}

fn make_task(val: Value) -> Task {
    serde_json::from_value(val).expect("Failed to create Task for test")
}

/// Build a valid minimal Jobspec to use as a base for setter/getter tests.
fn make_minimal_jobspec() -> Jobspec {
    let rv = make_resource_vertex(json!({"type": "node", "count": 1}));
    let task = make_task(json!({
        "command": ["sleep", "1"],
        "slot": "node",
        "count": {"per_slot": 1}
    }));
    Jobspec::try_from(RawJobspec {
        resources: vec![rv],
        tasks: vec![task],
        attributes: json!({"system": {}}),
        version: 1,
    })
    .expect("Failed to create minimal jobspec")
}

// =========================================================================
// TryFrom<RawJobspec> Validation
// =========================================================================

#[test]
fn try_from_valid_raw_jobspec_succeeds() {
    let _ = make_minimal_jobspec(); // Panics if invalid
}

#[test]
fn try_from_empty_resources_returns_error() {
    let task = make_task(json!({"command": ["ls"], "slot": "node", "count": {"per_slot": 1}}));
    let raw = RawJobspec {
        resources: vec![],
        tasks: vec![task],
        attributes: json!({"system": {}}),
        version: 1,
    };
    assert!(Jobspec::try_from(raw).is_err());
}

#[test]
fn try_from_empty_tasks_returns_error() {
    let rv = make_resource_vertex(json!({"type": "node", "count": 1}));
    let raw = RawJobspec {
        resources: vec![rv],
        tasks: vec![],
        attributes: json!({"system": {}}),
        version: 1,
    };
    assert!(Jobspec::try_from(raw).is_err());
}

#[test]
fn try_from_attributes_not_object_returns_error() {
    let rv = make_resource_vertex(json!({"type": "node", "count": 1}));
    let task = make_task(json!({"command": ["ls"], "slot": "node", "count": {"per_slot": 1}}));
    let raw = RawJobspec {
        resources: vec![rv],
        tasks: vec![task],
        attributes: json!(["system"]), // Array, not object
        version: 1,
    };
    assert!(Jobspec::try_from(raw).is_err());
}

#[test]
fn try_from_attributes_invalid_top_level_key_returns_error() {
    let rv = make_resource_vertex(json!({"type": "node", "count": 1}));
    let task = make_task(json!({"command": ["ls"], "slot": "node", "count": {"per_slot": 1}}));
    let raw = RawJobspec {
        resources: vec![rv],
        tasks: vec![task],
        attributes: json!({"system": {}, "custom_invalid": {}}),
        version: 1,
    };
    assert!(Jobspec::try_from(raw).is_err());
}

// =========================================================================
// get_attr / set_attr
// =========================================================================

#[test]
fn get_attr_less_than_two_components_returns_none() {
    let js = make_minimal_jobspec();
    assert!(js.get_attr("system").is_none());
}

#[test]
fn set_attr_less_than_two_components_returns_error() {
    let mut js = make_minimal_jobspec();
    assert!(js.set_attr("system", &json!("val")).is_err());
}

#[test]
fn set_attr_creates_nested_objects_and_get_attr_retrieves_them() {
    let mut js = make_minimal_jobspec();
    js.set_attr("system.deeply.nested.key", &42).unwrap();
    let val: i32 = js.get_attr_as("system.deeply.nested.key").unwrap();
    assert_eq!(val, 42);
}

#[test]
fn get_attr_mut_allows_mutation() {
    let mut js = make_minimal_jobspec();
    js.set_attr("system.test.val", &10).unwrap();
    let val_mut = js.get_attr_mut("system.test.val").unwrap();
    *val_mut = json!(20);
    let val: i32 = js.get_attr_as("system.test.val").unwrap();
    assert_eq!(val, 20);
}

// =========================================================================
// del_attr
// =========================================================================

#[test]
fn del_attr_less_than_two_components_returns_none() {
    let mut js = make_minimal_jobspec();
    assert!(js.del_attr("system", true).is_none());
}

#[test]
fn del_attr_removes_value_and_returns_it() {
    let mut js = make_minimal_jobspec();
    js.set_attr("system.test.key", &"value").unwrap();
    let removed = js.del_attr("system.test.key", false).unwrap();
    assert_eq!(removed, json!("value"));
    assert!(js.get_attr("system.test.key").is_none());
}

#[test]
fn del_attr_with_remove_empty_true_prunes_empty_parents() {
    let mut js = make_minimal_jobspec();
    js.set_attr("system.parent.child", &1).unwrap();
    js.del_attr("system.parent.child", true).unwrap();
    // "parent" should be removed because it became empty
    assert!(js.get_attr("system.parent").is_none());
}

#[test]
fn del_attr_with_remove_empty_false_leaves_empty_parents() {
    let mut js = make_minimal_jobspec();
    js.set_attr("system.parent.child", &1).unwrap();
    js.del_attr("system.parent.child", false).unwrap();
    // "parent" should still exist as an empty object
    let parent = js.get_attr("system.parent").unwrap();
    assert!(parent.is_object());
    assert!(parent.as_object().unwrap().is_empty());
}

// =========================================================================
// Shell Options & Duration
// =========================================================================

#[test]
fn set_and_get_attr_shell_options() {
    let mut js = make_minimal_jobspec();
    js.set_attr_shell_options("test.opt", &true).unwrap();
    let val: bool = js.get_attr_shell_options_as("test.opt").unwrap();
    assert!(val);
}

#[test]
fn set_and_get_duration() {
    let mut js = make_minimal_jobspec();
    js.set_duration(FluxDuration::Secs(120.5)).unwrap();
    let dur = js.duration_as().unwrap();
    assert!(matches!(dur, FluxDuration::Secs(s) if s == 120.5));
}

// =========================================================================
// Macro-generated getters and setters
// =========================================================================

#[test]
fn set_and_get_queue() {
    let mut js = make_minimal_jobspec();
    js.set_queue("debug").unwrap();
    assert_eq!(js.queue_as().unwrap(), "debug");
}

#[test]
fn set_and_get_cwd() {
    let mut js = make_minimal_jobspec();
    js.set_cwd(PathBuf::from("/tmp/work")).unwrap();
    assert_eq!(js.cwd_as().unwrap(), PathBuf::from("/tmp/work"));
}

#[test]
fn set_and_get_environment() {
    let mut js = make_minimal_jobspec();
    let env = vec![("FOO".to_string(), json!("BAR"))];
    js.set_environment(env).unwrap();
    let retrieved = js.environment_as().unwrap();
    assert_eq!(retrieved.get("FOO").unwrap(), &json!("BAR"));
}

// =========================================================================
// Unbuffered
// =========================================================================

#[test]
fn unbuffered_true_sets_correct_attributes() {
    let mut js = make_minimal_jobspec();
    js.unbuffered(true).unwrap();
    assert!(js.is_unbuffered());
    assert_eq!(
        js.get_attr_shell_options_as::<String>("output.stdout.buffer.type")
            .unwrap(),
        "none"
    );
    assert_eq!(
        js.get_attr_shell_options_as::<String>("output.stderr.buffer.type")
            .unwrap(),
        "none"
    );
    assert_eq!(
        js.get_attr_shell_options_as::<f64>("output.batch-timeout")
            .unwrap(),
        0.05
    );
    assert_eq!(
        js.get_attr_shell_options_as::<f64>("input.batch-timeout")
            .unwrap(),
        0.0
    );
}

#[test]
fn unbuffered_false_removes_attributes() {
    let mut js = make_minimal_jobspec();
    js.unbuffered(true).unwrap();
    js.unbuffered(false).unwrap();
    assert!(!js.is_unbuffered());
    assert!(
        js.get_attr_shell_options("output.stdout.buffer.type")
            .is_none()
    );
    assert!(js.get_attr_shell_options("output.batch-timeout").is_none());
}

// =========================================================================
// add_file
// =========================================================================

#[test]
fn add_file_creates_system_files_and_adds_entry() {
    let mut js = make_minimal_jobspec();
    let fileref = Fileref::create_empty_file_object("/tmp/test.txt", None, None, None);
    js.add_file(fileref).unwrap();

    let files = js.get_attr("system.files").unwrap().as_object().unwrap();
    assert!(files.contains_key("/tmp/test.txt"));
}

#[test]
fn add_file_appends_to_existing_system_files() {
    let mut js = make_minimal_jobspec();
    let f1 = Fileref::create_empty_file_object("/tmp/f1.txt", None, None, None);
    let f2 = Fileref::create_empty_file_object("/tmp/f2.txt", None, None, None);
    js.add_file(f1).unwrap();
    js.add_file(f2).unwrap();

    let files = js.get_attr("system.files").unwrap().as_object().unwrap();
    assert_eq!(files.len(), 2);
    assert!(files.contains_key("/tmp/f1.txt"));
    assert!(files.contains_key("/tmp/f2.txt"));
}

// =========================================================================
// iter_resources & resource_counts
// =========================================================================

#[test]
fn iter_resources_yields_in_dfs_order() {
    // Build a tree: node -> slot -> (core, gpu)
    let core = make_resource_vertex(json!({"type": "core", "count": 4}));
    let gpu = make_resource_vertex(json!({"type": "gpu", "count": 1}));
    let slot = make_resource_vertex(json!({
        "type": "slot",
        "count": 1,
        "label": "task",
        "with": [core, gpu]
    }));
    let node = make_resource_vertex(json!({
        "type": "node",
        "count": 1,
        "with": [slot]
    }));

    let js = Jobspec::try_from(RawJobspec {
        resources: vec![node],
        tasks: vec![make_task(json!({
            "command": ["ls"],
            "slot": "task",
            "count": {"per_slot": 1}
        }))],
        attributes: json!({"system": {}}),
        version: 1,
    })
    .unwrap();

    let types: Vec<&str> = js
        .iter_resources()
        .map(|v| v.resource_type.as_str())
        .collect();
    // DFS pre-order: node, slot, core, gpu
    assert_eq!(types, vec!["node", "slot", "core", "gpu"]);
}

#[test]
fn resource_counts_aggregates_all_variants_correctly() {
    let core = make_resource_vertex(json!({"type": "core", "count": "0-3"})); // Idset len 4
    let gpu = make_resource_vertex(json!({"type": "gpu", "count": {"min": 1, "max": 2}})); // Dict
    let node = make_resource_vertex(json!({
        "type": "node",
        "count": 2, // Integer
        "with": [core, gpu]
    }));

    let js = Jobspec::try_from(RawJobspec {
        resources: vec![node],
        tasks: vec![make_task(json!({
            "command": ["ls"],
            "slot": "node",
            "count": {"per_slot": 1}
        }))],
        attributes: json!({"system": {}}),
        version: 1,
    })
    .unwrap();

    let counts = js.resource_counts();
    assert_eq!(counts.get("node"), Some(&(2, 2)));
    assert_eq!(counts.get("core"), Some(&(4, 4)));
    assert_eq!(counts.get("gpu"), Some(&(1, 2)));
}

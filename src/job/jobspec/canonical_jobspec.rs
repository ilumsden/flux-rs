use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use pastey::paste;

use super::resource_vertex::ResourceVertex;
use super::task::Task;
use crate::duration::FluxDuration;
use crate::error::{FluxError, Result};
use crate::job::jobspec::{Fileref, ResourceCount};
use crate::utils::impl_serde_repr_str;

macro_rules! create_jobspec_getters_setters {
    ($attr_id:ident, $attr_path:expr, $as_type:ty, $set_type:ty) => {
        paste! {
            pub fn $attr_id(&self) -> Option<&Value> {
                self.get_attr($attr_path)
            }

            pub fn [<$attr_id _mut>](&mut self) -> Option<&mut Value> {
                self.get_attr_mut($attr_path)
            }

            pub fn [<$attr_id _as>](&self) -> Option<$as_type> {
                self.get_attr_as($attr_path)
            }

            pub fn [<set_ $attr_id>](&mut self, val: $set_type) -> Result<()> {
                self.set_attr($attr_path, val)
            }
        }
    };
    ($attr_id:ident, $attr_path:expr, $as_type:ty, as_ref $set_type:ty) => {
        paste! {
            pub fn $attr_id(&self) -> Option<&Value> {
                self.get_attr($attr_path)
            }

            pub fn [<$attr_id _mut>](&mut self) -> Option<&mut Value> {
                self.get_attr_mut($attr_path)
            }

            pub fn [<$attr_id _as>](&self) -> Option<$as_type> {
                self.get_attr_as($attr_path)
            }

            pub fn [<set_ $attr_id>](&mut self, val: impl AsRef<$set_type>) -> Result<()> {
                self.set_attr($attr_path, val.as_ref())
            }
        }
    };
    ($attr_id:ident, $attr_path:expr, $as_type:ty, into_iter $iter_type:ty, $collect_type:ty) => {
        paste! {
            pub fn $attr_id(&self) -> Option<&Value> {
                self.get_attr($attr_path)
            }

            pub fn [<$attr_id _mut>](&mut self) -> Option<&mut Value> {
                self.get_attr_mut($attr_path)
            }

            pub fn [<$attr_id _as>](&self) -> Option<$as_type> {
                self.get_attr_as($attr_path)
            }

            pub fn [<set_ $attr_id>]<I>(&mut self, val: I) -> Result<()>
            where I: IntoIterator<Item = $iter_type>,
            {
                let tmp_val: $collect_type = val.into_iter().collect();
                self.set_attr($attr_path, &tmp_val)
            }
        }
    };
}

// TODO uncomment if the utility macro is needed elsewhere
// pub(super) use create_jobspec_getters_setters;

#[derive(Serialize, Deserialize, Clone)]
#[serde(try_from = "RawJobspec")]
pub struct Jobspec {
    pub resources: Vec<ResourceVertex>,
    pub tasks: Vec<Task>,
    // pub attributes: HashMap<String, HashMap<String, Value>>,
    pub attributes: Value,
    pub version: u64,
}

impl Jobspec {
    pub fn get_attr(&self, attr_path: &str) -> Option<&Value> {
        if attr_path.split('.').count() < 2 {
            return None;
        }
        let pointer = format!("/{}", attr_path.replace('.', "/"));
        self.attributes.pointer(&pointer)
    }

    pub fn get_attr_mut(&mut self, attr_path: &str) -> Option<&mut Value> {
        if attr_path.split('.').count() < 2 {
            return None;
        }
        let pointer = format!("/{}", attr_path.replace('.', "/"));
        self.attributes.pointer_mut(&pointer)
    }

    pub fn get_attr_as<T: for<'de> Deserialize<'de>>(&self, attr_path: &str) -> Option<T> {
        serde_json::from_value(self.get_attr(attr_path)?.clone()).ok()
    }

    pub fn set_attr<T: Serialize + ?Sized>(&mut self, attr_path: &str, val: &T) -> Result<()> {
        let components: Vec<&str> = attr_path.split('.').collect();
        if components.len() < 2 {
            return Err(FluxError::Logic(
                "'attr_path' must include at least 2 components".to_string(),
            ));
        }
        // Get the leaf component name and all the preceding components.
        let (leaf_comp, path) = components.split_last().ok_or(FluxError::Logic("INTERNAL ERROR: somehow we cannot split the last element of the path from the rest despite prior checks passing".to_string()))?;
        let mut current = &mut self.attributes;
        for &comp in path {
            if !current.is_object() {
                *current = Value::Object(Map::new());
            }
            let obj = current.as_object_mut().ok_or(FluxError::Logic(format!(
                "Cannot get key {comp} as a JSON Object",
            )))?;
            current = obj.entry(comp).or_insert_with(|| Value::Object(Map::new()));
        }
        let last_key_in_path = *path.last().ok_or(FluxError::Logic("INTERNAL ERROR: somehow we cannot get the second-to-last element of the path despite prior checks passing".to_string()))?;
        if !current.is_object() {
            *current = Value::Object(Map::new());
        }
        current
            .as_object_mut()
            .ok_or(FluxError::Logic(format!(
                "Cannot get {last_key_in_path} as a JSON object",
            )))?
            .insert(leaf_comp.to_string(), serde_json::to_value(val)?);
        Ok(())
    }

    /// Perform a single step in the recursive-walk-and-remove over 'attributes'.
    ///
    /// This is a utility function to recursively walk through the 'Value' objects in 'attributes'
    /// until we reach the end of the path specified by 'components' or some other termination condition.
    fn del_attr_recurse(current: &mut Value, path: &[&str], remove_empty: bool) -> Option<Value> {
        let obj = current.as_object_mut()?;
        let (current_key, rest_of_path) = path.split_first()?;
        if rest_of_path.is_empty() {
            return obj.remove(*current_key);
        }
        let next_val = obj.get_mut(*current_key)?;
        let removed_val = Self::del_attr_recurse(next_val, rest_of_path, remove_empty);
        if remove_empty
            && let Some(child) = obj.get(*current_key)
            && child.as_object().is_some_and(|m| m.is_empty())
        {
            obj.remove(*current_key);
        }
        removed_val
    }

    pub fn del_attr(&mut self, attr_path: &str, remove_empty: bool) -> Option<Value> {
        let components: Vec<&str> = attr_path.split('.').collect();
        if components.len() < 2 {
            return None;
        }
        Self::del_attr_recurse(&mut self.attributes, &components, remove_empty)
    }

    pub fn get_attr_shell_options(&self, attr_path: &str) -> Option<&Value> {
        self.get_attr(&format!("system.shell.options.{attr_path}"))
    }

    pub fn get_attr_shell_options_mut(&mut self, attr_path: &str) -> Option<&mut Value> {
        self.get_attr_mut(&format!("system.shell.options.{attr_path}"))
    }

    pub fn get_attr_shell_options_as<T: for<'de> Deserialize<'de>>(
        &self,
        attr_path: &str,
    ) -> Option<T> {
        self.get_attr_as(&format!("system.shell.options.{attr_path}"))
    }

    pub fn set_attr_shell_options<T: Serialize + ?Sized>(
        &mut self,
        attr_path: &str,
        val: &T,
    ) -> Result<()> {
        self.set_attr(&format!("system.shell.options.{attr_path}"), val)
    }

    pub fn del_attr_shell_options(&mut self, attr_path: &str, remove_empty: bool) -> Option<Value> {
        self.del_attr(&format!("system.shell.options.{attr_path}"), remove_empty)
    }

    pub fn duration(&self) -> Option<&Value> {
        self.get_attr("system.duration")
    }

    pub fn duration_mut(&mut self) -> Option<&mut Value> {
        self.get_attr_mut("system.duration")
    }

    pub fn duration_as(&self) -> Option<FluxDuration<'_>> {
        let duration_secs: f64 = self.get_attr_as("system.duration")?;
        Some(FluxDuration::Secs(duration_secs))
    }

    pub fn set_duration(&mut self, val: FluxDuration) -> Result<()> {
        let duration_secs: f64 = val.try_into()?;
        self.set_attr("system.duration", &duration_secs)
    }

    create_jobspec_getters_setters!(queue, "system.queue", String, as_ref str);
    create_jobspec_getters_setters!(bank, "system.bank", String, as_ref str);
    create_jobspec_getters_setters!(cwd, "system.cwd", PathBuf, as_ref Path);
    create_jobspec_getters_setters!(environment, "system.environment", Map<String, Value>, into_iter (String, Value), Map<String, Value>);
    create_jobspec_getters_setters!(input, "system.shell.options.input.stdin.path", PathBuf, as_ref Path);
    create_jobspec_getters_setters!(stdin, "system.shell.options.input.stdin.path", PathBuf, as_ref Path);
    create_jobspec_getters_setters!(output, "system.shell.options.output.stdout.path", PathBuf, as_ref Path);
    create_jobspec_getters_setters!(stdout, "system.shell.options.output.stdout.path", PathBuf, as_ref Path);
    create_jobspec_getters_setters!(error, "system.shell.options.output.stderr.path", PathBuf, as_ref Path);
    create_jobspec_getters_setters!(stderr, "system.shell.options.output.stderr.path", PathBuf, as_ref Path);

    pub fn is_unbuffered(&self) -> bool {
        let mut val = true;
        if let Some(out_buf_type) =
            self.get_attr_shell_options_as::<String>("output.stdout.buffer.type")
        {
            val = val && out_buf_type == "none";
        } else {
            return false;
        }
        if let Some(err_buf_type) =
            self.get_attr_shell_options_as::<String>("output.stderr.buffer.type")
        {
            val = val && err_buf_type == "none";
        } else {
            return false;
        }
        val
    }

    pub fn unbuffered(&mut self, value: bool) -> Result<()> {
        if value {
            self.set_attr_shell_options("output.stdout.buffer.type", "none")?;
            self.set_attr_shell_options("output.stderr.buffer.type", "none")?;
            self.set_attr_shell_options("output.batch-timeout", &0.05)?;
            self.set_attr_shell_options("input.batch-timeout", &0.0)
        } else {
            self.del_attr_shell_options("output.stdout.buffer", true);
            self.del_attr_shell_options("output.stderr.buffer", true);
            if let Some(bt) = self.get_attr_shell_options_as::<f64>("output.batch-timeout")
                && bt == 0.05_f64
            {
                self.del_attr_shell_options("output.batch-timeout", true);
            }
            if let Some(bt) = self.get_attr_shell_options_as::<f64>("input.batch-timeout")
                && bt == 0.0_f64
            {
                self.del_attr_shell_options("input.batch-timeout", true);
            }
            Ok(())
        }
    }

    pub fn add_file(&mut self, fileref: Fileref) -> Result<()> {
        if let Some(files_value) = self.get_attr_mut("system.files") {
            let files_vec = files_value.as_object_mut().ok_or(FluxError::Logic("The 'system.files' attribute has already been added as a non-hash-map type. Delete the attribute and try again".to_string()))?;
            files_vec.insert(
                fileref.path.to_string_lossy().to_string(),
                serde_json::to_value(fileref)?,
            );
        } else {
            let mut new_files_map = Map::new();
            new_files_map.insert(
                fileref.path.to_string_lossy().to_string(),
                serde_json::to_value(fileref)?,
            );
            self.set_attr("system.files", &Value::Object(new_files_map))?;
        }
        Ok(())
    }

    /// Create an iterator over the resources in the Jobspec
    pub fn iter_resources(&self) -> ResourceIter<'_> {
        ResourceIter {
            _jobspec: self,
            iter_stack: vec![self.resources.iter()],
        }
    }

    pub fn resource_counts(&self) -> HashMap<&str, (usize, usize)> {
        let mut counts: HashMap<&str, (usize, usize)> = HashMap::new();
        for resource_vertex in self.iter_resources() {
            let count_entry_val = counts
                .entry(&resource_vertex.resource_type)
                .or_insert_with(|| (0, 0));
            let update_tup = match &resource_vertex.count {
                ResourceCount::Integer(v) => (*v, *v),
                ResourceCount::Idset(id) => (id.len(), id.len()),
                ResourceCount::Dict(count_dict) => {
                    let max_update = if let Some(max_val) = count_dict.max {
                        max_val
                    } else {
                        usize::MAX
                    };
                    (count_dict.min, max_update)
                }
            };
            if update_tup.0 == usize::MAX {
                count_entry_val.0 = update_tup.0;
            } else if count_entry_val.0 != usize::MAX {
                count_entry_val.0 += update_tup.0;
            }
            if update_tup.1 == usize::MAX {
                count_entry_val.1 = update_tup.1;
            } else if count_entry_val.1 != usize::MAX {
                count_entry_val.1 += update_tup.1;
            }
        }
        counts
    }
}

impl_serde_repr_str!(Jobspec);

/// Internal duplicate of the Jobspec struct used for validation
#[derive(Deserialize)]
pub(crate) struct RawJobspec {
    pub(crate) resources: Vec<ResourceVertex>,
    pub(crate) tasks: Vec<Task>,
    pub(crate) attributes: Value,
    pub(crate) version: u64,
}

impl TryFrom<RawJobspec> for Jobspec {
    type Error = FluxError;

    fn try_from(value: RawJobspec) -> std::prelude::v1::Result<Self, Self::Error> {
        if value.resources.is_empty() {
            return Err(FluxError::Logic(
                "The 'resources' field of the jobspec must have at least 1 element".to_string(),
            ));
        }
        if value.tasks.is_empty() {
            return Err(FluxError::Logic(
                "The 'tasks' field of the jobspec must have at least 1 element".to_string(),
            ));
        }
        if !value
            .attributes
            .as_object()
            .ok_or(FluxError::Logic(
                "The 'attributes' key must be a JSON object (or equivalent)".to_string(),
            ))?
            .keys()
            .all(|k| k == "system" || k == "user")
        {
            return Err(FluxError::Logic("The 'attributes' field of the jobspec must contain only top-level keys of 'system' or 'user', if provided".to_string()));
        }
        Ok(Self {
            resources: value.resources,
            tasks: value.tasks,
            attributes: value.attributes,
            version: value.version,
        })
    }
}

pub struct ResourceIter<'a> {
    _jobspec: &'a Jobspec,
    iter_stack: Vec<std::slice::Iter<'a, ResourceVertex>>,
}

impl<'a> Iterator for ResourceIter<'a> {
    type Item = &'a ResourceVertex;

    fn next(&mut self) -> Option<Self::Item> {
        // Repeatedly get the last iterator in 'iter_stack' until we find one
        // that isn't empty
        while let Some(curr_iter) = self.iter_stack.last_mut() {
            // If the current iterator is not empty, get the next vertex in the DFS from it.
            // Then, check if that next vertex has a 'with' field, and, if it does, push an iterator
            // over that 'with' field onto 'iter_stack'. Finally, return the next vertex.
            if let Some(next_vertex) = curr_iter.next() {
                if let Some(next_vertex_with) = &next_vertex.with {
                    self.iter_stack.push(next_vertex_with.iter());
                }
                return Some(next_vertex);
            } else {
                // If the current iterator is empty, pop it off 'iter_stack' and try again
                self.iter_stack.pop();
            }
        }
        // We hit here when 'iter_stack' is empty. This marks the end of the iteration, so
        // return None.
        None
    }
}

use std::env;
use std::fmt::Display;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};

use nix::sys::stat::Mode;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::duration::FluxDuration;
use crate::error::{FluxError, Result};
use crate::job::jobspec::{Fileref, ResourceVertex, Task};
use crate::job::jobspec::{ResourceCount, TaskCount};
use crate::utils::impl_serde_repr_str;

use super::canonical_jobspec::Jobspec;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PerResourceType {
    Node,
    Core,
}

#[derive(Serialize, Deserialize)]
#[serde(try_from = "Jobspec")]
pub struct JobspecV1(pub Jobspec);

impl JobspecV1 {
    pub fn new(resources: Vec<ResourceVertex>, tasks: Vec<Task>, attributes: Value) -> Self {
        Self(Jobspec {
            resources,
            tasks,
            attributes,
            version: 1,
        })
    }

    pub(crate) fn set_extra_options(&mut self, builder: BaseJobspecV1Builder<'_>) -> Result<()> {
        let label_io_val = builder.label_io.unwrap_or(false);
        let unbuffered_val = builder.unbuffered.unwrap_or(false);
        if let Some(dur) = builder.duration {
            self.set_duration(dur)?;
        }
        if let Some(env) = builder.environment {
            self.set_environment(env)?;
        } else {
            self.set_environment(env::vars().map(|(key, val)| (key, Value::from(val))))?;
        }
        if let Some(env_exp) = builder.env_expand {
            self.set_attr_shell_options("env-expand", &env_exp)?;
        }
        if let Some(curr_dir) = builder.cwd {
            self.set_cwd(curr_dir)?;
        }
        if let Some(rlim) = builder.rlimits {
            self.set_attr_shell_options("rlimit", &rlim)?;
        }
        if let Some(job_name) = builder.name {
            self.set_attr("system.job.name", &job_name)?;
        }
        if let Some(stdin) = builder.input {
            self.set_input(stdin)?;
        }
        if let Some(stdout) = builder.output {
            if stdout != Path::new("none") && stdout != Path::new("kvs") {
                self.set_output(stdout)?;
                if label_io_val {
                    self.set_attr_shell_options("output.stdout.label", &true)?;
                }
            }
        }
        if let Some(stderr) = builder.error {
            if stderr != Path::new("none") && stderr != Path::new("kvs") {
                self.set_error(stderr)?;
                if label_io_val {
                    self.set_attr_shell_options("output.stderr.label", &true)?;
                }
            }
        }
        if unbuffered_val {
            self.unbuffered(true)?;
        }
        if let Some(queue_val) = builder.queue {
            self.set_queue(queue_val)?;
        }
        if let Some(bank_val) = builder.bank {
            self.set_bank(bank_val)?;
        }
        Ok(())
    }

    pub fn per_resource<'a, Cmd>(command: Cmd) -> PerResourceBuilder<'a>
    where
        Cmd: IntoIterator,
        Cmd::Item: ToString,
    {
        PerResourceBuilder {
            command: command.into_iter().map(|v| v.to_string()).collect(),
            exclusive: false,
            ncores: None,
            nnodes: None,
            per_resource_type: None,
            per_resource_count: None,
            gpus_per_node: None,
            base: BaseJobspecV1Builder::new(),
        }
    }

    pub fn from_command<'a, Cmd>(command: Cmd) -> FromCommandBuilder<'a>
    where
        Cmd: IntoIterator,
        Cmd::Item: ToString,
    {
        FromCommandBuilder {
            command: command.into_iter().map(|v| v.to_string()).collect(),
            num_tasks: 1,
            cores_per_task: 1,
            exclusive: false,
            gpus_per_task: None,
            num_nodes: None,
            base: BaseJobspecV1Builder::new(),
        }
    }

    pub fn from_nest_command<'a, Cmd>(command: Cmd) -> FromNestCommandBuilder<'a>
    where
        Cmd: IntoIterator,
        Cmd::Item: ToString,
    {
        FromNestCommandBuilder {
            command: command.into_iter().map(|v| v.to_string()).collect(),
            num_slots: 1,
            cores_per_slot: 1,
            exclusive: false,
            gpus_per_slot: None,
            num_nodes: None,
            broker_opts: None,
            conf: None,
            base: BaseJobspecV1Builder::new(),
        }
    }

    pub fn from_batch_command<'a>(
        script: impl ToString,
        jobname: impl ToString,
    ) -> FromBatchCommandBuilder<'a> {
        let mut builder = FromBatchCommandBuilder {
            script: script.to_string(),
            num_slots: 1,
            cores_per_slot: 1,
            exclusive: false,
            args: None,
            gpus_per_slot: None,
            num_nodes: None,
            broker_opts: None,
            conf: None,
            base: BaseJobspecV1Builder::new(),
        };
        builder.base = builder.base.name(jobname);
        builder
    }
}

impl Deref for JobspecV1 {
    type Target = Jobspec;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for JobspecV1 {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl_serde_repr_str!(JobspecV1);

impl TryFrom<Jobspec> for JobspecV1 {
    type Error = FluxError;

    fn try_from(value: Jobspec) -> Result<Self> {
        let system_map = if let Some(sys_map) = value.attributes.get("system") {
            sys_map
        } else {
            return Err(FluxError::Logic(
                "A V1 Jobspec requires the 'system' key in attributes".to_string(),
            ));
        };
        if let Some(dur_val) = system_map.get("duration") {
            if !dur_val.is_number() {
                return Err(FluxError::Logic(
                    "The 'system.duration' key in a V1 Jobspec's attributes must be a number"
                        .to_string(),
                ));
            }
        } else {
            return Err(FluxError::Logic(
                "A V1 Jobspec requires the 'system.duration' key in attributes".to_string(),
            ));
        };
        for task in &value.tasks {
            match &task.count {
                TaskCount::PerSlot(_) | TaskCount::Total(_) => {},
                TaskCount::PerResource(_) | TaskCount::Extension(_, _) => return Err(FluxError::Logic("A V1 Jobspec only accepts 'per slot' or 'total' resource counts for a task".to_string())),
            }
        }
        for resource in value.iter_resources() {
            if !matches!(resource.count, ResourceCount::Integer(_)) {
                return Err(FluxError::Logic(
                    "A V1 Jobspec requires the 'count' field for each resource to be an integer"
                        .to_string(),
                ));
            }
        }
        Ok(Self(value))
    }
}

pub struct BaseJobspecV1Builder<'a> {
    pub(crate) duration: Option<FluxDuration<'a>>,
    pub(crate) environment: Option<Map<String, Value>>,
    pub(crate) env_expand: Option<Map<String, Value>>,
    pub(crate) cwd: Option<PathBuf>,
    pub(crate) rlimits: Option<Map<String, Value>>,
    pub(crate) name: Option<String>,
    pub(crate) input: Option<PathBuf>,
    pub(crate) output: Option<PathBuf>,
    pub(crate) error: Option<PathBuf>,
    pub(crate) label_io: Option<bool>,
    pub(crate) unbuffered: Option<bool>,
    pub(crate) queue: Option<String>,
    pub(crate) bank: Option<String>,
}

impl<'a> BaseJobspecV1Builder<'a> {
    pub fn new() -> Self {
        Self {
            duration: None,
            environment: None,
            env_expand: None,
            cwd: None,
            rlimits: None,
            name: None,
            input: None,
            output: None,
            error: None,
            label_io: None,
            unbuffered: None,
            queue: None,
            bank: None,
        }
    }

    pub fn duration(mut self, dur: FluxDuration<'a>) -> Self {
        self.duration = Some(dur);
        self
    }

    pub fn environment<I>(mut self, environ: I) -> Self
    where
        I: IntoIterator<Item = (String, Value)>,
    {
        self.environment = Some(environ.into_iter().collect());
        self
    }

    pub fn env_expand<I>(mut self, env_exp: I) -> Self
    where
        I: IntoIterator<Item = (String, Value)>,
    {
        self.env_expand = Some(env_exp.into_iter().collect());
        self
    }

    pub fn cwd(mut self, curr_dir: impl AsRef<Path>) -> Self {
        self.cwd = Some(curr_dir.as_ref().to_path_buf());
        self
    }

    pub fn rlimits<I>(mut self, rlim: I) -> Self
    where
        I: IntoIterator<Item = (String, Value)>,
    {
        self.rlimits = Some(rlim.into_iter().collect());
        self
    }

    pub fn name(mut self, n: impl ToString) -> Self {
        self.name = Some(n.to_string());
        self
    }

    pub fn input(mut self, stdin: impl AsRef<Path>) -> Self {
        self.input = Some(stdin.as_ref().to_path_buf());
        self
    }

    pub fn output(mut self, stdout: impl AsRef<Path>) -> Self {
        self.output = Some(stdout.as_ref().to_path_buf());
        self
    }

    pub fn error(mut self, stderr: impl AsRef<Path>) -> Self {
        self.error = Some(stderr.as_ref().to_path_buf());
        self
    }

    pub fn label_io(mut self, use_label_io: bool) -> Self {
        self.label_io = Some(use_label_io);
        self
    }

    pub fn unbuffered(mut self, use_unbuffered: bool) -> Self {
        self.unbuffered = Some(use_unbuffered);
        self
    }

    pub fn queue(mut self, queue_val: impl ToString) -> Self {
        self.queue = Some(queue_val.to_string());
        self
    }

    pub fn bank(mut self, bank_val: impl ToString) -> Self {
        self.bank = Some(bank_val.to_string());
        self
    }
}

macro_rules! delegate_to_base {
    ($base_field:ident, $method_name:ident, $val_type:ty) => {
        pub fn $method_name(mut self, value: $val_type) -> Self {
            self.$base_field = self.$base_field.$method_name(value);
            self
        }
    };
    ($base_field:ident, $struct_lifetime:lifetime) => {
        delegate_to_base!($base_field, duration, FluxDuration<$struct_lifetime>);
        delegate_to_base!(
            $base_field,
            environment,
            impl IntoIterator<Item = (String, Value)>
        );
        delegate_to_base!(
            $base_field,
            env_expand,
            impl IntoIterator<Item = (String, Value)>
        );
        delegate_to_base!($base_field, cwd, impl AsRef<Path>);
        delegate_to_base!(
            $base_field,
            rlimits,
            impl IntoIterator<Item = (String, Value)>
        );
        delegate_to_base!($base_field, name, impl ToString);
        delegate_to_base!($base_field, input, impl AsRef<Path>);
        delegate_to_base!($base_field, output, impl AsRef<Path>);
        delegate_to_base!($base_field, error, impl AsRef<Path>);
        delegate_to_base!($base_field, label_io, bool);
        delegate_to_base!($base_field, unbuffered, bool);
        delegate_to_base!($base_field, queue, impl ToString);
        delegate_to_base!($base_field, bank, impl ToString);
    };
}

pub struct PerResourceBuilder<'a> {
    pub(crate) command: Vec<String>,
    pub(crate) exclusive: bool,
    pub(crate) ncores: Option<usize>,
    pub(crate) nnodes: Option<usize>,
    pub(crate) per_resource_type: Option<PerResourceType>,
    pub(crate) per_resource_count: Option<usize>,
    pub(crate) gpus_per_node: Option<usize>,
    pub(crate) base: BaseJobspecV1Builder<'a>,
}

impl<'a> PerResourceBuilder<'a> {
    pub fn exclusive(mut self, exc: bool) -> Self {
        self.exclusive = exc;
        self
    }

    pub fn ncores(mut self, num_cores: usize) -> Self {
        self.ncores = Some(num_cores);
        self
    }

    pub fn nnodes(mut self, num_nodes: usize) -> Self {
        self.nnodes = Some(num_nodes);
        self
    }

    pub fn per_resource_type_and_count(
        mut self,
        resource_type: PerResourceType,
        resource_count: usize,
    ) -> Self {
        self.per_resource_type = Some(resource_type);
        self.per_resource_count = Some(resource_count);
        self
    }

    pub fn gpus_per_node(mut self, num_gpus: usize) -> Self {
        self.gpus_per_node = Some(num_gpus);
        self
    }

    delegate_to_base!(base, 'a);

    pub fn build_jobspec(self) -> Result<JobspecV1> {
        // TODO in the future, consider adding the Typestate pattern to push some of these checks off to compile time.
        let mut per_resource: Option<Value> = None;
        if self.per_resource_type.is_some() && self.per_resource_count.is_none() {
            return Err(FluxError::Logic("The 'per_resource_count' parameter must be specified when 'per_resource_type' is specified".to_string()));
        }
        if self.per_resource_type.is_none() && self.per_resource_count.is_some() {
            return Err(FluxError::Logic("The 'per_resource_type' parameter must be specified when 'per_resource_count' is specified".to_string()));
        }
        if let Some(pr_type) = self.per_resource_type {
            if let Some(pr_count) = self.per_resource_count {
                let mut pr_map = Map::new();
                pr_map.insert("type".to_string(), serde_json::to_value(pr_type)?);
                pr_map.insert("count".to_string(), Value::from(pr_count));
                per_resource = Some(Value::Object(pr_map));
            }
        }
        if self.gpus_per_node.is_some() && self.nnodes.is_none() {
            return Err(FluxError::Logic(
                "The 'nnodes' parameter may not be None when 'gpus_per_node' is not None"
                    .to_string(),
            ));
        }
        if self.exclusive && self.nnodes.is_none() {
            return Err(FluxError::Logic(
                "The 'exclusive' paramter may only be set with a node count".to_string(),
            ));
        }
        let nslots: usize;
        let slot_size: usize;
        if self.nnodes.is_some() && self.ncores.is_some() {
            let num_nodes = self.nnodes.unwrap();
            let num_cores = self.ncores.unwrap();
            if num_cores < num_nodes {
                return Err(FluxError::Logic(
                    "The 'ncores' parameter cannot be less than 'nnodes' when both are provided"
                        .to_string(),
                ));
            }
            if num_cores % num_nodes != 0 {
                return Err(FluxError::Logic("The 'ncores' parameter must be evenly divisible by 'nnodes' when both are provided".to_string()));
            }
            nslots = 1;
            slot_size = num_cores / num_nodes;
        } else if self.ncores.is_some() {
            let num_cores = self.ncores.unwrap();
            nslots = num_cores;
            slot_size = 1;
        } else if self.nnodes.is_some() {
            if !self.exclusive {
                return Err(FluxError::Logic(
                    "Specifying 'nnodes' also requires 'ncores' or 'exclusive'".to_string(),
                ));
            }
            nslots = 1;
            slot_size = 1;
        } else {
            return Err(FluxError::Logic(
                "At least one of 'nnodes' or 'ncores' are required".to_string(),
            ));
        }
        let mut children = vec![ResourceVertex {
            resource_type: "core".to_string(),
            count: ResourceCount::Integer(slot_size),
            unit: None,
            exclusive: None,
            with: None,
            label: None,
            id: None,
        }];
        if let Some(gpus) = self.gpus_per_node {
            children.push(ResourceVertex {
                resource_type: "gpu".to_string(),
                count: ResourceCount::Integer(gpus),
                unit: None,
                exclusive: None,
                with: None,
                label: None,
                id: None,
            })
        }
        let slot = ResourceVertex {
            resource_type: "slot".to_string(),
            count: ResourceCount::Integer(nslots),
            unit: None,
            exclusive: None,
            with: Some(children),
            label: Some("task".to_string()),
            id: None,
        };
        let resources = if let Some(num_nodes) = self.nnodes {
            vec![ResourceVertex {
                resource_type: "node".to_string(),
                count: ResourceCount::Integer(num_nodes),
                unit: None,
                exclusive: Some(self.exclusive),
                with: Some(vec![slot]),
                label: None,
                id: None,
            }]
        } else {
            vec![slot]
        };
        let tasks = vec![Task {
            command: self.command,
            slot: "task".to_string(),
            count: TaskCount::PerSlot(1),
            attributes: None,
            distribution: None,
        }];
        let mut attributes_map = Map::new();
        let sys_attrs = attributes_map
            .entry("system".to_string())
            .or_insert(Value::Object(Map::new()));
        sys_attrs
            .as_object_mut()
            .ok_or(FluxError::Logic(
                "Cannot get the 'system' key as a JSON object".to_string(),
            ))?
            .insert("duration".to_string(), Value::from(0_u64));
        let mut jobspec = JobspecV1::new(resources, tasks, Value::Object(attributes_map));
        if let Some(pr) = per_resource {
            jobspec.set_attr_shell_options("per-resource", &pr)?;
        }
        jobspec.set_extra_options(self.base)?;
        Ok(jobspec)
    }
}

pub struct FromCommandBuilder<'a> {
    pub(crate) command: Vec<String>,
    pub(crate) num_tasks: usize,
    pub(crate) cores_per_task: usize,
    pub(crate) exclusive: bool,
    pub(crate) gpus_per_task: Option<usize>,
    pub(crate) num_nodes: Option<usize>,
    pub(crate) base: BaseJobspecV1Builder<'a>,
}

impl<'a> FromCommandBuilder<'a> {
    pub fn num_tasks(mut self, ntasks: usize) -> Self {
        self.num_tasks = ntasks;
        self
    }

    pub fn cores_per_task(mut self, cores: usize) -> Self {
        self.cores_per_task = cores;
        self
    }

    pub fn exclusive(mut self, exc: bool) -> Self {
        self.exclusive = exc;
        self
    }

    pub fn gpus_per_task(mut self, gpus: usize) -> Self {
        self.gpus_per_task = Some(gpus);
        self
    }

    pub fn num_nodes(mut self, nnodes: usize) -> Self {
        self.num_nodes = Some(nnodes);
        self
    }

    delegate_to_base!(base, 'a);

    pub fn build_jobspec(self) -> Result<JobspecV1> {
        if let Some(nnodes) = self.num_nodes {
            if nnodes > self.num_tasks {
                return Err(FluxError::Logic(
                    "Node count must be greater than task count".to_string(),
                ));
            }
        } else if self.exclusive {
            return Err(FluxError::Logic(
                "The 'exclusive' paramter can only be set with a node count".to_string(),
            ));
        }
        let mut children = vec![ResourceVertex {
            resource_type: "core".to_string(),
            count: ResourceCount::Integer(self.cores_per_task),
            unit: None,
            exclusive: None,
            with: None,
            label: None,
            id: None,
        }];
        if let Some(gpus) = self.gpus_per_task {
            if gpus > 0 {
                children.push(ResourceVertex {
                    resource_type: "gpu".to_string(),
                    count: ResourceCount::Integer(gpus),
                    unit: None,
                    exclusive: None,
                    with: None,
                    label: None,
                    id: None,
                });
            }
        }
        let task_count: TaskCount;
        let resources: Vec<ResourceVertex> = if let Some(nnodes) = self.num_nodes {
            let num_slots = (self.num_tasks as f64 / nnodes as f64).ceil() as usize;
            if self.num_tasks % nnodes != 0 {
                task_count = TaskCount::Total(self.num_tasks as u64);
            } else {
                task_count = TaskCount::PerSlot(1);
            }
            let slot = ResourceVertex {
                resource_type: "slot".to_string(),
                count: ResourceCount::Integer(num_slots),
                unit: None,
                exclusive: None,
                with: Some(children),
                label: Some("task".to_string()),
                id: None,
            };
            vec![ResourceVertex {
                resource_type: "node".to_string(),
                count: ResourceCount::Integer(nnodes),
                unit: None,
                exclusive: Some(self.exclusive),
                with: Some(vec![slot]),
                label: None,
                id: None,
            }]
        } else {
            task_count = TaskCount::PerSlot(1);
            vec![ResourceVertex {
                resource_type: "slot".to_string(),
                count: ResourceCount::Integer(self.num_tasks),
                unit: None,
                exclusive: None,
                with: Some(children),
                label: Some("task".to_string()),
                id: None,
            }]
        };
        let tasks = vec![Task {
            command: self.command,
            slot: "task".to_string(),
            count: task_count,
            attributes: None,
            distribution: None,
        }];
        let mut attributes = Map::new();
        let sys_attrs = attributes
            .entry("system".to_string())
            .or_insert(Value::Object(Map::new()));
        sys_attrs
            .as_object_mut()
            .ok_or(FluxError::Logic(
                "Cannot get the 'system' key as a JSON object".to_string(),
            ))?
            .insert("duration".to_string(), Value::from(0_u64));
        let mut jobspec = JobspecV1::new(resources, tasks, Value::Object(attributes));
        jobspec.set_extra_options(self.base)?;
        Ok(jobspec)
    }
}

pub struct FromNestCommandBuilder<'a> {
    pub(crate) command: Vec<String>,
    pub(crate) num_slots: usize,
    pub(crate) cores_per_slot: usize,
    pub(crate) exclusive: bool,
    pub(crate) gpus_per_slot: Option<usize>,
    pub(crate) num_nodes: Option<usize>,
    pub(crate) broker_opts: Option<Vec<String>>,
    pub(crate) conf: Option<String>,
    pub(crate) base: BaseJobspecV1Builder<'a>,
}

impl<'a> FromNestCommandBuilder<'a> {
    pub fn num_slots(mut self, nslots: usize) -> Self {
        self.num_slots = nslots;
        self
    }

    pub fn cores_per_slot(mut self, cores: usize) -> Self {
        self.cores_per_slot = cores;
        self
    }

    pub fn exclusive(mut self, exc: bool) -> Self {
        self.exclusive = exc;
        self
    }

    pub fn gpus_per_slot(mut self, gpus: usize) -> Self {
        self.gpus_per_slot = Some(gpus);
        self
    }

    pub fn num_nodes(mut self, nnodes: usize) -> Self {
        self.num_nodes = Some(nnodes);
        self
    }

    pub fn broker_opts<I>(mut self, opts: I) -> Self
    where
        I: IntoIterator,
        I::Item: Display,
    {
        self.broker_opts = Some(opts.into_iter().map(|v| v.to_string()).collect());
        self
    }

    pub fn conf(mut self, conf_contents: impl ToString) -> Self {
        self.conf = Some(conf_contents.to_string());
        self
    }

    delegate_to_base!(base, 'a);

    pub fn build_jobspec(mut self) -> Result<JobspecV1> {
        let mut broker_opts = if let Some(bopts) = self.broker_opts {
            bopts
        } else {
            Vec::new()
        };
        let conf_fileref = if let Some(conf_contents) = self.conf {
            broker_opts.push("-c{{tmpdir}}/conf.json".to_string());
            if conf_contents.contains('\n') {
                Some(Fileref::create_text_content_object(
                    "conf.json",
                    conf_contents,
                    None,
                    None,
                    None,
                ))
            } else {
                Some(Fileref::create_from_file("conf.json", &conf_contents)?)
            }
        } else {
            None
        };
        let mut command = vec!["flux".to_string(), "broker".to_string()];
        command.append(&mut broker_opts);
        command.append(&mut self.command);
        let mut jobspec = FromCommandBuilder {
            command: command,
            num_tasks: self.num_slots,
            cores_per_task: self.cores_per_slot,
            exclusive: self.exclusive,
            gpus_per_task: self.gpus_per_slot,
            num_nodes: self.num_nodes,
            base: self.base,
        }
        .build_jobspec()?;
        jobspec.set_attr_shell_options("per-resource.type", "node")?;
        jobspec.set_attr_shell_options("mpi", "none")?;
        jobspec.set_attr_shell_options("exit-timeout", "none")?;
        if let Some(conf_ref) = conf_fileref {
            jobspec.add_file(conf_ref)?;
        }
        Ok(jobspec)
    }
}

pub struct FromBatchCommandBuilder<'a> {
    script: String,
    num_slots: usize,
    cores_per_slot: usize,
    exclusive: bool,
    args: Option<Vec<String>>,
    gpus_per_slot: Option<usize>,
    num_nodes: Option<usize>,
    broker_opts: Option<Vec<String>>,
    conf: Option<String>,
    base: BaseJobspecV1Builder<'a>,
}

impl<'a> FromBatchCommandBuilder<'a> {
    pub fn args<I>(mut self, arg_iter: I) -> Self
    where
        I: IntoIterator,
        I::Item: ToString,
    {
        self.args = Some(arg_iter.into_iter().map(|v| v.to_string()).collect());
        self
    }

    pub fn num_slots(mut self, nslots: usize) -> Self {
        self.num_slots = nslots;
        self
    }

    pub fn cores_per_slot(mut self, cores: usize) -> Self {
        self.cores_per_slot = cores;
        self
    }

    pub fn gpus_per_slot(mut self, gpus: usize) -> Self {
        self.gpus_per_slot = Some(gpus);
        self
    }

    pub fn num_nodes(mut self, nnodes: usize) -> Self {
        self.num_nodes = Some(nnodes);
        self
    }

    pub fn broker_opts<I>(mut self, opts: I) -> Self
    where
        I: IntoIterator,
        I::Item: ToString,
    {
        self.broker_opts = Some(opts.into_iter().map(|v| v.to_string()).collect());
        self
    }

    pub fn exclusive(mut self, exc: bool) -> Self {
        self.exclusive = exc;
        self
    }

    pub fn conf(mut self, conf_contents_or_path: impl ToString) -> Self {
        self.conf = Some(conf_contents_or_path.to_string());
        self
    }

    delegate_to_base!(base, 'a);

    pub fn build_jobspec(self) -> Result<JobspecV1> {
        if !self.script.starts_with("#!") {
            return Err(FluxError::Logic("The 'script' parameter must be the contents of the batch script starting with a shebang (i.e., '#!')".to_string()));
        }
        let mut args = if let Some(js_args) = self.args {
            js_args
        } else {
            Vec::new()
        };
        let mut command = vec!["{{tmpdir}}/script".to_string()];
        command.append(&mut args);
        let mut jobspec = FromNestCommandBuilder {
            command,
            num_slots: self.num_slots,
            cores_per_slot: self.cores_per_slot,
            exclusive: self.exclusive,
            gpus_per_slot: self.gpus_per_slot,
            num_nodes: self.num_nodes,
            broker_opts: self.broker_opts,
            conf: self.conf,
            base: self.base,
        }
        .build_jobspec()?;
        let script_fileref = Fileref::create_text_content_object(
            "script",
            self.script,
            Some(Mode::S_IRWXU),
            None,
            None,
        );
        jobspec.add_file(script_fileref)?;
        Ok(jobspec)
    }
}

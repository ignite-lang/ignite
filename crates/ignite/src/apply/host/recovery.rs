//! Startup recovery: verify every retained revision and admitted context
//! against this host, then resume the active or committed revision once no
//! earlier owned instance survives.

use std::collections::BTreeSet;
use std::path::Path;

use ignite_eval::Json;

use super::admission::retained_plan;
use super::controller::process_identity;
use super::journal::FORMAT;
use super::{Host, read_json};
use crate::apply::{Failure, fail, object, plan::Plan, required, text};

fn generation_process_exists(root: &Path) -> Result<bool, Failure> {
    let generations = root.join("revisions");
    for entry in std::fs::read_dir("/proc")? {
        let entry = entry?;
        if !entry
            .file_name()
            .to_string_lossy()
            .bytes()
            .all(|byte| byte.is_ascii_digit())
        {
            continue;
        }
        if let Ok(executable) = std::fs::read_link(entry.path().join("exe"))
            && executable.starts_with(&generations)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

impl Host {
    /// Re-verify each retained revision's journal and load the retained
    /// context of every other admitted operation.
    pub(super) fn verify_history(&self) -> Result<(), Failure> {
        let root = &self.root;
        let mut checked_plans = BTreeSet::new();
        if root.join("revisions").is_dir() {
            for entry in std::fs::read_dir(root.join("revisions"))? {
                let path = entry?.path();
                if !path.is_dir() {
                    continue;
                }
                let metadata = read_json(&path.join("context.json"))?;
                if required(&metadata, "formatVersion")? != FORMAT
                    || required(&metadata, "controllerAbi")? != crate::apply::plan::CONTROLLER_ABI
                {
                    return Err(fail(3, "unsupported historical host/reducer version"));
                }
                let id = required(&metadata, "planId")?;
                checked_plans.insert(id.to_string());
                let value = read_json(&root.join("plans").join(format!("{id}.json")))?;
                let plan = Plan::parse(&value).map_err(|reason| fail(3, reason))?;
                let context = crate::apply::revision::Context::load(retained_plan(root, &plan)?)?;
                crate::apply::revision::verify(context, &path)?;
            }
        }
        for (id, receipt) in &self.receipts {
            if receipt.get("noop").and_then(Json::as_bool) == Some(true)
                || checked_plans.contains(id)
            {
                continue;
            }
            let value = read_json(&root.join("plans").join(format!("{id}.json")))?;
            let plan = Plan::parse(&value).map_err(|reason| fail(3, reason))?;
            crate::apply::revision::Context::load(retained_plan(root, &plan)?)?;
        }
        Ok(())
    }

    /// Resume the active or committed revision unless recovery is required;
    /// `intents` are the operations with a journaled effect.
    pub(super) fn resume(&mut self, intents: &BTreeSet<String>) -> Result<(), Failure> {
        let resume = if self.recovery_reason.is_none() {
            self.active.clone().or_else(|| self.committed.clone())
        } else {
            None
        };
        if let Some(id) = resume {
            let live_owned = self
                .owned
                .values()
                .chain(self.retiring.values())
                .any(|owned| {
                    let pid = owned.get("pid").and_then(Json::as_i64);
                    let instance = owned.get("instance").and_then(Json::as_str);
                    pid.zip(instance).is_some_and(|(pid, instance)| {
                        process_identity(pid).as_deref() == Some(instance)
                    })
                });
            if live_owned || generation_process_exists(&self.root)? {
                self.reject_operation(&id, "an earlier owned process instance still exists; listener adoption requires reconciliation")?;
            } else {
                self.owned.clear();
                self.retiring.clear();
                self.append(
                    "reconciledRevision",
                    object([
                        ("planId", text(&id)),
                        ("priorIntents", Json::Bool(intents.contains(&id))),
                        (
                            "observation",
                            text(
                                "previous owned instances ended; no retained-generation process remains",
                            ),
                        ),
                    ]),
                )?;
                let value = read_json(&self.root.join("plans").join(format!("{id}.json")))?;
                self.start(Plan::parse(&value).map_err(|reason| fail(3, reason))?)?;
            }
        }
        Ok(())
    }
}

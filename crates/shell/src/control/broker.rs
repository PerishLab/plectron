use super::addon::{Addon, Context, Notice, Outcome, Request, Surface};
use super::receipt::{Admission, CAPACITY, Ledger, Receipt};
use super::wire::{Inbound, Reply, decode, encode};
use std::collections::{BTreeSet, VecDeque};
use webengine::Fault;

const OUTBOUND: usize = 256;

pub struct Broker {
    ledger: Ledger,
    addons: Vec<Box<dyn Addon>>,
    policy: Option<BTreeSet<String>>,
    outbound: VecDeque<String>,
    active: bool,
}

impl Default for Broker {
    fn default() -> Self {
        Self::new(CAPACITY)
    }
}

impl Broker {
    pub fn new(capacity: usize) -> Self {
        Self {
            ledger: Ledger::new(capacity),
            addons: Vec::new(),
            policy: None,
            outbound: VecDeque::new(),
            active: false,
        }
    }

    pub fn register(&mut self, addon: Box<dyn Addon>) -> Result<(), Fault> {
        let name = addon.namespace();
        if name.is_empty() || name.chars().any(char::is_control) {
            return Err(Fault("addon namespace is invalid".into()));
        }
        if self
            .addons
            .iter()
            .any(|existing| existing.namespace() == name)
        {
            return Err(Fault(format!("duplicate addon namespace: {name}")));
        }
        self.addons.push(addon);
        Ok(())
    }

    pub fn allow(&mut self, command: impl Into<String>) {
        self.policy
            .get_or_insert_with(BTreeSet::new)
            .insert(command.into());
    }

    pub fn ledger(&self) -> &Ledger {
        &self.ledger
    }

    pub fn pending(&self) -> usize {
        self.outbound.len()
    }

    pub fn peek(&self) -> Option<&str> {
        self.outbound.front().map(String::as_str)
    }

    pub fn pop(&mut self) -> Option<String> {
        self.outbound.pop_front()
    }

    pub fn receive(&mut self, raw: &str, surface: &mut dyn Surface) -> Receipt {
        if self.active {
            let receipt = Receipt::rejected("", "", "reentrant control execution refused");
            return self.emit(receipt, (false, false));
        }
        let message = match decode(raw) {
            Ok(inbound) => inbound,
            Err((id, reason)) => {
                return self.emit(Receipt::rejected(id, "", reason), (false, false));
            }
        };
        match message {
            Inbound::Query { id } => self.query(&id),
            Inbound::Cancel { id } => self.cancel(&id, surface),
            Inbound::Command {
                id,
                target,
                payload,
            } => {
                let request = Request {
                    id: &id,
                    command: &target,
                    payload: &payload,
                };
                self.execute(&request, surface)
            }
        }
    }

    pub fn poll(&mut self, surface: &mut dyn Surface) {
        if self.active {
            return;
        }
        self.active = true;
        let mut notices = Vec::new();
        for addon in &mut self.addons {
            let mut context = Context::new(surface);
            notices.extend(addon.poll(&mut context));
        }
        self.active = false;
        for notice in notices {
            self.notice(notice);
        }
    }

    fn query(&mut self, id: &str) -> Receipt {
        match self.ledger.query(id).cloned() {
            Some(receipt) => self.emit(receipt, (false, true)),
            None => self.emit(
                Receipt::rejected(id, "", "command_id not found"),
                (false, false),
            ),
        }
    }

    fn cancel(&mut self, id: &str, surface: &mut dyn Surface) -> Receipt {
        let Some(existing) = self.ledger.query(id).cloned() else {
            return self.emit(
                Receipt::rejected(id, "", "command_id not found"),
                (false, false),
            );
        };
        if existing.status.terminal() {
            return self.emit(existing, (false, true));
        }
        let Some(index) = self.owner(&existing.target) else {
            return self.emit(existing, (false, true));
        };
        self.active = true;
        let mut context = Context::new(surface);
        let outcome = self.addons[index].cancel(id, &mut context);
        self.active = false;
        let receipt = match outcome {
            Some(result) => self.apply(id, result).unwrap_or(existing),
            None => existing,
        };
        self.emit(receipt, (false, true))
    }

    fn execute(&mut self, request: &Request<'_>, surface: &mut dyn Surface) -> Receipt {
        let id = request.id;
        let target = request.command;
        if let Some(existing) = self.ledger.query(id).cloned() {
            return self.emit(existing, (true, true));
        }
        let Some(index) = self.authorized(target) else {
            let receipt = Receipt::rejected(id, target, format!("unauthorized command: {target}"));
            return self.emit(receipt, (false, false));
        };
        let admission = match self.ledger.admit(id, target) {
            Ok(outcome) => outcome,
            Err(reason) => return self.emit(Receipt::rejected(id, target, reason), (false, false)),
        };
        match admission {
            Admission::Duplicate(receipt) => self.emit(receipt, (true, true)),
            Admission::Refused(receipt) => self.emit(receipt, (false, false)),
            Admission::Execute(_) => {
                let _ = self.ledger.start(id);
                self.active = true;
                let mut context = Context::new(surface);
                let outcome = self.addons[index].execute(request, &mut context);
                self.active = false;
                let receipt = self
                    .apply(id, outcome)
                    .unwrap_or_else(|reason| Receipt::rejected(id, target, reason));
                self.emit(receipt, (false, true))
            }
        }
    }

    fn notice(&mut self, notice: Notice) {
        if let Ok(receipt) = self.apply(&notice.command, notice.outcome) {
            let _ = self.emit(receipt, (false, true));
        }
    }

    fn apply(&mut self, id: &str, outcome: Outcome) -> Result<Receipt, String> {
        match outcome {
            Outcome::Running(Some(update)) => self.ledger.progress(id, update),
            Outcome::Running(None) => self
                .ledger
                .query(id)
                .cloned()
                .ok_or_else(|| "command_id not found".to_string()),
            Outcome::Completed(value) => self.ledger.complete(id, value),
            Outcome::Rejected(reason) => self.ledger.reject(id, reason),
            Outcome::Unknown(reason) => self.ledger.uncertain(id, reason),
        }
    }

    fn authorized(&self, target: &str) -> Option<usize> {
        if let Some(allowed) = &self.policy
            && !allowed.contains(target)
        {
            return None;
        }
        self.owner(target)
    }

    fn owner(&self, target: &str) -> Option<usize> {
        self.addons.iter().position(|addon| addon.allows(target))
    }

    fn emit(&mut self, receipt: Receipt, flags: (bool, bool)) -> Receipt {
        let reply = Reply {
            receipt: receipt.clone(),
            duplicate: flags.0,
            found: flags.1,
        };
        if self.outbound.len() >= OUTBOUND {
            self.outbound.pop_front();
        }
        self.outbound.push_back(encode(&reply));
        receipt
    }
}

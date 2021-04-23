use crate::functions::FUNCTIONS;
use crate::{OpenFilter, PreFilter};
use alloc::string::{String, ToString};
use alloc::{collections::BTreeMap, vec::Vec};

pub struct Definition {
    pub name: String,
    pub args: Vec<String>,
    pub term: PreFilter,
}

pub struct Definitions(Vec<Definition>);

pub struct Module(Definitions);

impl Definitions {
    pub fn new(defs: Vec<Definition>) -> Self {
        Self(defs)
    }
}

impl Module {
    pub fn new(defs: Definitions) -> Self {
        Self(defs)
    }
}

pub struct Main {
    pub defs: Definitions,
    pub term: PreFilter,
}

impl Main {
    pub fn open(self, module: Module) -> Result<OpenFilter, ()> {
        let filter = self.term;
        let mut fns: BTreeMap<(String, usize), _> = FUNCTIONS
            .iter()
            .map(|(name, args, f)| ((name.to_string(), *args), f.clone().into()))
            .collect();
        for def in module.0 .0.into_iter().chain(self.defs.0.into_iter()) {
            let open = def.term.open(&def.args, &|name, args| {
                fns.get(&(name, args)).cloned().ok_or(())
            });
            fns.insert((def.name, def.args.len()), open.unwrap());
        }
        filter.open(&[], &|name, args| fns.get(&(name, args)).cloned().ok_or(()))
    }
}

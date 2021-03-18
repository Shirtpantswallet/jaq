use crate::filter::{Filter, FilterT};
use crate::val::{RVals, Val};
use crate::Error;
use alloc::{boxed::Box, rc::Rc};

#[derive(Debug)]
pub enum NewFunc {
    Not,
    All,
    Any,
    Add,
    Length,
    Map(Box<Filter>),
}

#[derive(Debug)]
pub enum RefFunc {
    Empty,
    Select(Box<Filter>),
    Recurse(Box<Filter>),
}

impl NewFunc {
    fn run_single(&self, v: Rc<Val>) -> Result<Val, Error> {
        use NewFunc::*;
        match self {
            Any => Ok(Val::Bool(v.iter()?.any(|v| v.as_bool()))),
            All => Ok(Val::Bool(v.iter()?.all(|v| v.as_bool()))),
            Not => Ok(Val::Bool(!v.as_bool())),
            Add => v
                .iter()?
                .map(|x| (*x).clone())
                .try_fold(Val::Null, |acc, x| acc + x),
            Length => Ok(Val::Num(v.len()?)),
            Map(f) => Ok(Val::Arr(
                v.iter()?.flat_map(|x| f.run(x)).collect::<Result<_, _>>()?,
            )),
        }
    }
}

impl FilterT for NewFunc {
    fn run(&self, v: Rc<Val>) -> RVals {
        Box::new(core::iter::once(self.run_single(v).map(Rc::new)))
    }
}

impl FilterT for RefFunc {
    fn run(&self, v: Rc<Val>) -> RVals {
        use RefFunc::*;
        match self {
            Empty => Box::new(core::iter::empty()),
            Select(f) => Box::new(f.run(Rc::clone(&v)).filter_map(move |y| match y {
                Ok(y) => {
                    if y.as_bool() {
                        Some(Ok(Rc::clone(&v)))
                    } else {
                        None
                    }
                }
                Err(e) => Some(Err(e)),
            })),
            Recurse(f) => Box::new(crate::Recurse::new(f, v)),
        }
    }
}

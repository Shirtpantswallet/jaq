use crate::{ClosedFilter, Error, Filter, RValRs, Val, ValR};
use alloc::{boxed::Box, rc::Rc, string::ToString, vec::Vec};
use core::convert::TryFrom;

pub const FUNCTIONS: &[(&str, usize, Builtin<usize>)] = &[
    // new filters
    ("null", 0, Builtin::New(New::Null)),
    ("true", 0, Builtin::New(New::True)),
    ("false", 0, Builtin::New(New::False)),
    ("not", 0, Builtin::New(New::Not)),
    ("add", 0, Builtin::New(New::Add)),
    ("length", 0, Builtin::New(New::Length)),
    ("type", 0, Builtin::New(New::Type)),
    // referencing filters
    ("empty", 0, Builtin::Ref(Ref::Empty)),
    ("repeat", 1, Builtin::Ref(Ref::Repeat(0))),
    ("first", 1, Builtin::Ref(Ref::First(0))),
    ("last", 1, Builtin::Ref(Ref::Last(0))),
    ("limit", 2, Builtin::Ref(Ref::Limit(0, 1))),
    ("recurse", 1, Builtin::Ref(Ref::Recurse(0))),
    ("fold", 3, Builtin::Ref(Ref::Fold(0, 1, 2))),
];

#[derive(Clone, Debug)]
pub enum Builtin<F> {
    New(New),
    Ref(Ref<F>),
}

#[derive(Clone, Debug)]
pub enum New {
    Null,
    True,
    False,
    Not,
    Add,
    Length,
    Type,
}

#[derive(Clone, Debug)]
pub enum Ref<F> {
    Empty,
    Repeat(F),
    First(F),
    Last(F),
    Limit(F, F),
    Recurse(F),
    Fold(F, F, F),
}

impl Builtin<Box<ClosedFilter>> {
    pub fn run(&self, v: Rc<Val>) -> RValRs {
        match self {
            Self::New(n) => Box::new(core::iter::once(n.run(v).map(Rc::new))),
            Self::Ref(r) => r.run(v),
        }
    }
}

impl New {
    fn run(&self, v: Rc<Val>) -> ValR {
        use New::*;
        match self {
            Null => Ok(Val::Null),
            True => Ok(Val::Bool(true)),
            False => Ok(Val::Bool(false)),
            Not => Ok(Val::Bool(!v.as_bool())),
            Add => v
                .iter()?
                .map(|x| (*x).clone())
                .try_fold(Val::Null, |acc, x| acc + x),
            Length => Ok(Val::Num(v.len()?)),
            Type => Ok(Val::Str(v.typ().to_string())),
        }
    }
}

impl Ref<Box<ClosedFilter>> {
    fn run(&self, v: Rc<Val>) -> RValRs {
        use core::iter::{empty, once};
        match self {
            Self::Empty => Box::new(empty()),
            Self::Repeat(f) => Box::new(f.run(v).collect::<Vec<_>>().into_iter().cycle()),
            Self::First(f) => Box::new(f.run(v).take(1)),
            Self::Last(f) => match f.run(v).try_fold(None, |_, x| Ok(Some(x?))) {
                Ok(Some(y)) => Box::new(once(Ok(y))),
                Ok(None) => Box::new(empty()),
                Err(e) => Box::new(once(Err(e))),
            },
            Self::Limit(n, f) => {
                let n = n.run(Rc::clone(&v)).map(|n| usize::try_from(&*n?));
                Box::new(n.flat_map(move |n| match n {
                    Ok(n) => Box::new(f.run(Rc::clone(&v)).take(n as usize)),
                    Err(e) => Box::new(once(Err(e))) as Box<dyn Iterator<Item = _>>,
                }))
            }
            Self::Recurse(f) => Box::new(crate::Recurse::new(f, v)),
            Self::Fold(xs, init, f) => {
                let mut xs = xs.run(Rc::clone(&v));
                let init: Result<Vec<_>, _> = init.run(Rc::clone(&v)).collect();
                match init.and_then(|init| xs.try_fold(init, |acc, x| f.fold_step(acc, x?))) {
                    Ok(y) => Box::new(y.into_iter().map(Ok)),
                    Err(e) => Box::new(once(Err(e))),
                }
            }
        }
    }
}

impl ClosedFilter {
    fn fold_step(&self, acc: Vec<Rc<Val>>, x: Rc<Val>) -> Result<Vec<Rc<Val>>, Error> {
        acc.into_iter()
            .map(|acc| {
                let obj = [("acc".to_string(), acc), ("x".to_string(), Rc::clone(&x))];
                Val::Obj(Vec::from(obj).into_iter().collect())
            })
            .flat_map(|obj| self.run(Rc::new(obj)))
            .collect()
    }
}

impl<F> Builtin<F> {
    pub fn map<G>(self, m: &impl Fn(F) -> G) -> Builtin<G> {
        match self {
            Self::New(n) => Builtin::New(n),
            Self::Ref(r) => Builtin::Ref(r.map(m)),
        }
    }
}

impl<N> Builtin<Box<Filter<N>>> {
    pub fn try_map<F, M, E>(self, m: &F) -> Result<Builtin<Box<Filter<M>>>, E>
    where
        F: Fn(N) -> Result<Filter<M>, E>,
    {
        match self {
            Builtin::New(n) => Ok(Builtin::New(n)),
            Builtin::Ref(r) => Ok(Builtin::Ref(r.try_map(m)?)),
        }
    }
}

impl<F> Ref<F> {
    fn map<G>(self, m: &impl Fn(F) -> G) -> Ref<G> {
        use Ref::*;
        match self {
            Empty => Empty,
            Repeat(f) => Repeat(m(f)),
            First(f) => First(m(f)),
            Last(f) => Last(m(f)),
            Limit(n, f) => Limit(m(n), m(f)),
            Recurse(f) => Recurse(m(f)),
            Fold(xs, init, f) => Fold(m(xs), m(init), m(f)),
        }
    }
}

impl<N> Ref<Box<Filter<N>>> {
    fn try_map<F, M, E>(self, m: &F) -> Result<Ref<Box<Filter<M>>>, E>
    where
        F: Fn(N) -> Result<Filter<M>, E>,
    {
        let m = |f: Filter<N>| f.try_map(m).map(Box::new);
        use Ref::*;
        match self {
            Empty => Ok(Empty),
            Repeat(f) => Ok(Repeat(m(*f)?)),
            First(f) => Ok(First(m(*f)?)),
            Last(f) => Ok(Last(m(*f)?)),
            Limit(n, f) => Ok(Limit(m(*n)?, m(*f)?)),
            Recurse(f) => Ok(Recurse(m(*f)?)),
            Fold(xs, init, f) => Ok(Fold(m(*xs)?, m(*init)?, m(*f)?)),
        }
    }
}

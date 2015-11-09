// Copyright 2015 Adrien Champion. See the COPYRIGHT file at the top-level
// directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

extern crate term ;
extern crate system as sys ;

use std::fmt ;
// use std::sync::{ RwLock } ;
use std::sync::mpsc::Sender ;
use std::collections::HashMap ;

use term::{
  Offset, Term, Sym, Factory
} ;

use sys::Prop ;

// pub type InvariantSet = RwLock<HashMap<Sym, Term>> ;

// pub type Cex = HashMap<(Var, Offset), Cst> ;

/** Message from kino to the techniques. */
pub enum MsgDown {
  Invariants(Sym, Vec<Term>),
  Forget(Sym),
}

/** Enumerates the techniques. */
#[derive(Debug, Clone, Copy)]
pub enum Technique {
  /** Bounded model checking. */
  Bmc,
  /** Induction. */
  Ind,
}
impl Technique {
  /** A string representation of a technique. */
  pub fn to_str(& self) -> & str {
    use Technique::* ;
    match * self {
      Bmc => "bmc",
      Ind => "ind",
    }
  }
}

/** Info a technique can communicate. */
pub enum Info {
  At(Offset)
}
impl fmt::Display for Info {
  fn fmt(& self, fmt: & mut fmt::Formatter) -> fmt::Result {
    match * self {
      Info::At(ref o) => write!(fmt, "at {}", o)
    }
  }
}

/** Message from the techniques to kino. */
pub enum MsgUp {
  /** Invariants discovered. */
  Invariants,
  /** Not implemented. */
  Inimplemented,
  /** Technique is done. */
  Done(Technique, Info),
  /** Log message. */
  Bla(Technique, String),
  /** Error message. */
  Error(Technique, String),
  /** Some properties were proved. */
  Proved(Vec<Sym>, Technique, Info),
  /** Some properties were falsified. */
  Disproved(Vec<Sym>, Technique, Info),
}

/** Used by the techniques to communicate with kino. */
pub struct Event {
  /** Sender to kino, */
  r: Sender<MsgUp>,
  /** Identifier of the technique. */
  t: Technique,
  /** Term factory. */
  f: Factory,
  /** K-true properties. */
  k_true: HashMap<Sym, Option<Offset>>,
}
impl Event {
  /** Creates a new `Event`. */
  pub fn mk(
    r: Sender<MsgUp>, t: Technique, f: Factory, props: & [Prop]
  ) -> Self {
    let mut k_true = HashMap::with_capacity(props.len()) ;
    for prop in props {
      match k_true.insert(prop.sym().clone(), None) {
        None => (),
        Some(_) => unreachable!(),
      }
    } ;
    Event { r: r, t: t, f: f, k_true: k_true }
  }
  /** Sends a done message upwards. */
  pub fn done(& self, info: Info) {
    self.r.send(
      MsgUp::Done(self.t, info)
    ).unwrap()
  }
  /** Sends a done message upwards. */
  pub fn done_at(& self, o: & Offset) {
    self.done(Info::At(o.clone()))
  }
  /** Sends a proved message upwards. */
  pub fn proved(& self, props: Vec<Sym>, info: Info) {
    self.r.send(
      MsgUp::Proved(props, self.t, info)
    ).unwrap()
  }
  /** Sends a falsification message upwards. */
  pub fn disproved(& self, props: Vec<Sym>, info: Info) {
    self.r.send(
      MsgUp::Disproved(props, self.t, info)
    ).unwrap()
  }
  /** Sends a falsification message upwards. */
  pub fn disproved_at(& self, props: Vec<Sym>, o: & Offset) {
    self.disproved(props, Info::At(o.clone()))
  }
  /** Sends a log message upwards. */
  pub fn log(& self, s: & str) {
    self.r.send(
      MsgUp::Bla(self.t, s.to_string())
    ).unwrap()
  }
  /** Sends a log message upwards. */
  pub fn error(& self, s: & str) {
    self.r.send(
      MsgUp::Error(self.t, s.to_string())
    ).unwrap()
  }
  /** The factory in an `Event`. */
  pub fn factory(& self) -> & Factory {
    & self.f
  }
  /** Returns the offset a property is k_true for. */
  #[inline(always)]
  pub fn k_true(& self, p: & Sym) -> & Option<Offset> {
    match self.k_true.get(p) {
      Some(res) => res,
      None => panic!("[event.k_true] unknown property"),
    }
  }
}



// pub struct EventBuilder {
//   r: Sender<MsgUp>,
// }
// impl EventBuilder {
//   pub fn mk(r: Sender<MsgUp>) -> Self { EventBuilder { r: r } }
//   pub fn event(self, t: Technique) -> Event { Event::mk(self.r, t) }
// }


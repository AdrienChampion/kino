// Copyright 2015 Adrien Champion. See the COPYRIGHT file at the top-level
// directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![deny(missing_docs)]

//! Template-based invariant generation.

#[macro_use]
extern crate error_chain ;
extern crate term ;
extern crate system ;
#[macro_use]
extern crate common ;
extern crate unroll ;

use std::sync::Arc ;
use std::fmt::Display ;

use term::{
  Factory, Term, TermSet,
  Cst, Bool, Int, Rat, Offset,
  // STerm, STermSet
} ;
use term::tmp::{
  TmpTerm, TmpTermSet,
  // TmpTermMap
} ;

use system::{ Sys, Prop } ;

use common::{ SolverTrait, CanRun } ;
use common::msg::Event ;
use common::conf ;
use common::errors::* ;

pub mod eval ;
pub mod mine ;
pub mod chain ;
pub mod graph ;
use graph::CanLog ;
pub mod lsd ;


/// Invgen technique.
pub struct Tig ;
unsafe impl Send for Tig {}
impl CanRun<conf::Tig> for Tig {
  fn id(& self) -> common::Tek { common::Tek::Tig }

  fn run(
    & self, conf: Arc<conf::Tig>, sys: Sys, _: Vec<Prop>, mut event: Event
  ) {

    let mut solver_conf = conf.smt().clone().default().print_success() ;
    match * conf.smt_cmd() {
      None => (),
      Some(ref cmd) => solver_conf = solver_conf.cmd(cmd.clone()),
    } ;

    mk_two_solver_run!(
      solver_conf, conf.smt_log(), "tig", event.factory(),
      (solver_1 "base", solver_2 "step") => {
        if let Some(ref dir) = * conf.graph_log() {
          use std::fs::DirBuilder ;
          let mut db = DirBuilder::new() ;
          db.recursive(true) ;
          log_try!(
            event, db.create(dir)
            => "while creating directory `{}` for graph logging", dir
          ) ;
          invgen(
            conf.clone(), solver_1, solver_2, sys, & mut event,
            |graph, tag1, tag2| graph.log_to(
              & format!("{}/graph_{}_{}.dot", dir, tag1, tag2)
            )
          )
        } else {
          invgen(
            conf.clone(), solver_1, solver_2, sys, & mut event,
            |_, _, _| Ok(())
          )
        }
      },
      err => event.error(err)
    )
  }
}


/// Runs invgen.
fn invgen<
  'a, S: SolverTrait<'a>,
  GraphLog: Fn(
    & graph::Learner< graph::Graph<Bool> >, & str, & str
  ) -> Res<()>
>(
  conf: Arc<conf::Tig>, solver_1: S, solver_2: S, sys: Sys, event: & mut Event,
  graph_log: GraphLog
) {
  use std::time::Instant ;
  use lsd::top_only::* ;

  let max_k = * conf.max() ;
  let unroll_step = * conf.step_roll() ;

  let factory = solver_1.parser().clone() ;

  // let mut blah = format!("{} ->", rep) ;
  // for t in class.iter() {
  //   blah = format!("{}\n    {}", blah, t)
  // } ;
  // event.log(& blah) ;

  // event.log("creating graph") ;
  // let mut graph = PartialGraph::of(
  //   & factory,
  //   Graph::<Bool>::mk(sys.clone(), rep, class),
  //   & (* conf)
  // ) ;
  let mut graph = graph::mk_bool_learner(
    sys.clone(), factory, & * conf
  ) ;

  event.log(
    & format!("running with {} candidate terms", graph.len() + 1)
  ) ;

  // event.log("creating base checker") ;
  let mut base = log_try!(
    event, Base::mk(& sys, solver_1, 0) => "while creating base checker"
  ) ;

  // event.log("creating step checker") ;
  let mut step = {
    let mut base = log_try!(
      event, Base::mk(& sys, solver_2, 0)
      => "while creating base checker to create step checker"
    ) ;
    log_try!(
      event, base.unroll()
      => "while unrolling base checker to create step checker"
    ) ;
    log_try!(
      event, base.to_step()
      => "while turning base checker into step checker"
    )
  } ;

  let mut cnt = 1 ;
  // let mut known_invars = TmpTermSet::with_capacity(107) ;

  'work: while max_k.map_or(true, |max| cnt <= max) {

    let mut is_done = false ;
    let mut inner_cnt = 0 ;
    let start = Instant::now() ;

    event.log( & format!("stabilizing at {}...", cnt) ) ;

    'stabilize: while ! is_done {
      is_done = log_try!(
        event, graph.stabilize_next_class_and_edges(
          & mut base, & mut step, event,
          & graph_log, & format!("{}_{}", cnt, inner_cnt)
        ) => "while stabilizing at {}", cnt
      ) ;

      // try_error!(
      //   base.restart(), event, "while restarting base at {}", cnt - 1
      // ) ;
      // try_error!(
      //   step.restart(), event, "while restarting step at {}", cnt - 1
      // ) ;

      inner_cnt += 1
    }

    let time = Instant::now() - start ;

    event.log(
      & format!(
        "graph stablized at {} in {}.{}",
        cnt, time.as_secs(), time.subsec_nanos()
      )
    ) ;

    graph.clear() ;

    log_try!(
      event, graph.k_split_all(& mut base, & mut step, event)
      => "while splitting all at {}", cnt
    ) ;

    log_try!(
      event, base.restart() => "while restarting base at {}", cnt - 1
    ) ;
    let base_len = log_try!(
      event, base.unroll() => "while unrolling base to {}", cnt
    ) ;
    debug_assert!( base_len == cnt ) ;

    cnt += 1 ;

    log_try!(
      event, step.restart() => "while restarting step at {}", cnt - 1
    ) ;
    if unroll_step {
      let step_len = log_try!(
        event, step.unroll() => "while unrolling step to {}", cnt
      ) ;
      debug_assert!( step_len == cnt )
    }

  }

  event.done_at( & Offset::of_int(cnt) ) ;
}








/** Trait for domains.

Domains define the type of the values the candidate terms evaluate to and a
total order relation used for the edges in the graph. */
pub trait Domain : PartialEq + Eq + PartialOrd + Ord + Clone + Display {
  /// A value from a constant.
  fn of_cst(& Cst) -> Res<Self> ;
  /// Creates a term encoding a relation between terms.
  fn mk_cmp(& Term, & Term) -> Option<TmpTerm> ;
  /// Creates a term encoding an equality between terms.
  fn mk_eq(& Term, & Term) -> Option<TmpTerm> ;
  /// Creates a term encoding a relation between terms.
  fn insert_cmp<
    Ignore: Fn(& TmpTerm) -> bool
  >(lhs: & Term, rhs: & Term, set: & mut TmpTermSet, ignore: Ignore) {
    if let Some( term ) = Self::mk_cmp(lhs, rhs) {
      if ! ignore(& term) { set.insert(term) ; () }
    }
  }
  /// Creates a term encoding an equality between terms.
  fn insert_eq<
    Ignore: Fn(& TmpTerm) -> bool
  >(lhs: & Term, rhs: & Term, set: & mut TmpTermSet, ignore: Ignore) {
    if let Some( term ) = Self::mk_eq(lhs, rhs) {
      if ! ignore(& term) { set.insert(term) ; () }
    }
  }
  /// Chooses a representative in a set, removes it from the set.
  fn choose_rep(& Factory, TermSet) -> Res<(Term, TermSet)> ;
}
impl Domain for Bool {
  fn of_cst(cst: & Cst) -> Res<Self> {
    match * cst.get() {
      ::term::real_term::Cst::Bool(b) => Ok(b),
      ref cst => bail!("[Bool::of_cst] unexpected constant {}", cst),
    }
  }
  fn mk_cmp(lhs: & Term, rhs: & Term) -> Option<TmpTerm> {
    if ! lhs.is_false() && ! rhs.is_true() {
      Some( TmpTerm::mk_term_impl(lhs.clone(), rhs.clone()) )
    } else {
      None
    }
  }
  fn mk_eq(lhs: & Term, rhs: & Term) -> Option<TmpTerm> {
    Some( TmpTerm::mk_term_eq(lhs.clone(), rhs.clone()) )
  }
  fn choose_rep(
    factory: & Factory, mut set: TermSet
  ) -> Res<(Term, TermSet)> {
    use term::CstMaker ;
    let tru = factory.cst(true) ;
    let was_there = set.remove(& tru) ;
    if was_there {
      Ok( (tru, set) )
    } else {
      let rep = match set.iter().next() {
        Some(rep) => rep.clone(),
        None => bail!(
          "[Bool::choose_rep] cannot choose representative of empty set"
        ),
      } ;
      let was_there = set.remove(& rep) ;
      debug_assert!( was_there ) ;
      Ok( (rep, set) )
    }
  }
}
impl Domain for Int  {
  fn of_cst(cst: & Cst) -> Res<Self> {
    match * cst.get() {
      ::term::real_term::Cst::Int(ref i) => Ok(i.clone()),
      ref cst => bail!("[Int::of_cst] unexpected constant {}", cst),
    }
  }
  fn mk_cmp(lhs: & Term, rhs: & Term) -> Option<TmpTerm> {
    Some( TmpTerm::mk_term_le(lhs.clone(), rhs.clone()) )
  }
  fn mk_eq(lhs: & Term, rhs: & Term) -> Option<TmpTerm> {
    Some( TmpTerm::mk_term_eq(lhs.clone(), rhs.clone()) )
  }
  fn choose_rep(_: & Factory, mut set: TermSet) -> Res<(Term, TermSet)> {
    let rep = match set.iter().next() {
      Some(rep) => rep.clone(),
      None => bail!(
        "[Int::choose_rep] cannot choose representative of empty set"
      ),
    } ;
    let was_there = set.remove(& rep) ;
    debug_assert!( was_there ) ;
    Ok( (rep, set) )
  }
}
impl Domain for Rat  {
  fn of_cst(cst: & Cst) -> Res<Self> {
    match * cst.get() {
      ::term::real_term::Cst::Rat(ref r) => Ok(r.clone()),
      ref cst => bail!("[Rat::of_cst] unexpected constant {}", cst),
    }
  }
  fn mk_cmp(lhs: & Term, rhs: & Term) -> Option<TmpTerm> {
    Some( TmpTerm::mk_term_le(lhs.clone(), rhs.clone()) )
  }
  fn mk_eq(lhs: & Term, rhs: & Term) -> Option<TmpTerm> {
    Some( TmpTerm::mk_term_eq(lhs.clone(), rhs.clone()) )
  }
  fn choose_rep(_: & Factory, mut set: TermSet) -> Res<(Term, TermSet)> {
    let rep = match set.iter().next() {
      Some(rep) => rep.clone(),
      None => bail!(
        "[Rat::choose_rep] cannot choose representative of empty set"
      ),
    } ;
    let was_there = set.remove(& rep) ;
    debug_assert!( was_there ) ;
    Ok( (rep, set) )
  }
}
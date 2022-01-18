use crate::arena::*;
use crate::atom_table::*;
use crate::fixtures::*;
use crate::forms::*;
use crate::instructions::*;
use crate::iterators::*;
use crate::types::*;

use crate::parser::ast::*;
use crate::parser::rug::ops::PowAssign;
use crate::parser::rug::{Assign, Integer, Rational};

use crate::machine::machine_errors::*;
use crate::machine::machine_indices::*;

use ordered_float::*;

use std::cell::Cell;
use std::cmp::{max, min, Ordering};
use std::convert::TryFrom;
use std::f64;
use std::num::FpCategory;
use std::ops::Div;
use std::rc::Rc;
use std::vec::Vec;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ArithmeticTerm {
    Reg(RegType),
    Interm(usize),
    Number(Number),
}

impl ArithmeticTerm {
    pub(crate) fn interm_or(&self, interm: usize) -> usize {
        if let &ArithmeticTerm::Interm(interm) = self {
            interm
        } else {
            interm
        }
    }
}

impl Default for ArithmeticTerm {
    fn default() -> Self {
        ArithmeticTerm::Number(Number::default())
    }
}

#[derive(Debug)]
pub(crate) struct ArithInstructionIterator<'a> {
    state_stack: Vec<TermIterState<'a>>,
}

pub(crate) type ArithCont = (Code, Option<ArithmeticTerm>);

impl<'a> ArithInstructionIterator<'a> {
    fn push_subterm(&mut self, lvl: Level, term: &'a Term) {
        self.state_stack
            .push(TermIterState::subterm_to_state(lvl, term));
    }

    fn from(term: &'a Term) -> Result<Self, ArithmeticError> {
        let state = match term {
            Term::AnonVar => return Err(ArithmeticError::UninstantiatedVar),
            Term::Clause(cell, name, terms) => match ClauseType::from(*name, terms.len()) {
                ct @ ClauseType::Named(..) => {
                    Ok(TermIterState::Clause(Level::Shallow, 0, cell, ct, terms))
                }
                ClauseType::Inlined(InlinedClauseType::IsFloat(_)) => {
                    let ct = ClauseType::Named(1, atom!("float"), CodeIndex::default());
                    Ok(TermIterState::Clause(Level::Shallow, 0, cell, ct, terms))
                }
                _ => Err(ArithmeticError::NonEvaluableFunctor(
                    Literal::Atom(*name),
                    terms.len(),
                )),
            }?,
            Term::Literal(cell, cons) => TermIterState::Literal(Level::Shallow, cell, cons),
            Term::Cons(..) | Term::PartialString(..) => {
                return Err(ArithmeticError::NonEvaluableFunctor(
                    Literal::Atom(atom!(".")),
                    2,
                ))
            }
            Term::Var(cell, var) => TermIterState::Var(Level::Shallow, cell, var.clone()),
        };

        Ok(ArithInstructionIterator {
            state_stack: vec![state],
        })
    }
}

#[derive(Debug)]
pub(crate) enum ArithTermRef<'a> {
    Literal(&'a Literal),
    Op(Atom, usize), // name, arity.
    Var(&'a Cell<VarReg>, Rc<String>),
}

impl<'a> Iterator for ArithInstructionIterator<'a> {
    type Item = Result<ArithTermRef<'a>, ArithmeticError>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(iter_state) = self.state_stack.pop() {
            match iter_state {
                TermIterState::AnonVar(_) => return Some(Err(ArithmeticError::UninstantiatedVar)),
                TermIterState::Clause(lvl, child_num, cell, ct, subterms) => {
                    let arity = subterms.len();

                    if child_num == arity {
                        return Some(Ok(ArithTermRef::Op(ct.name(), arity)));
                    } else {
                        self.state_stack.push(TermIterState::Clause(
                            lvl,
                            child_num + 1,
                            cell,
                            ct,
                            subterms,
                        ));

                        self.push_subterm(lvl, &subterms[child_num]);
                    }
                }
                TermIterState::Literal(_, _, c) => return Some(Ok(ArithTermRef::Literal(c))),
                TermIterState::Var(_, cell, var) => {
                    return Some(Ok(ArithTermRef::Var(cell, var.clone())));
                }
                _ => {
                    return Some(Err(ArithmeticError::NonEvaluableFunctor(
                        Literal::Atom(atom!(".")),
                        2,
                    )));
                }
            };
        }

        None
    }
}

#[derive(Debug)]
pub(crate) struct ArithmeticEvaluator<'a> {
    bindings: &'a AllocVarDict,
    interm: Vec<ArithmeticTerm>,
    interm_c: usize,
}

pub(crate) trait ArithmeticTermIter<'a> {
    type Iter: Iterator<Item = Result<ArithTermRef<'a>, ArithmeticError>>;

    fn iter(self) -> Result<Self::Iter, ArithmeticError>;
}

impl<'a> ArithmeticTermIter<'a> for &'a Term {
    type Iter = ArithInstructionIterator<'a>;

    fn iter(self) -> Result<Self::Iter, ArithmeticError> {
        ArithInstructionIterator::from(self)
    }
}

fn push_literal(interm: &mut Vec<ArithmeticTerm>, c: &Literal) -> Result<(), ArithmeticError> {
    match c {
        Literal::Fixnum(n) => interm.push(ArithmeticTerm::Number(Number::Fixnum(*n))),
        Literal::Integer(n) => interm.push(ArithmeticTerm::Number(Number::Integer(*n))),
        Literal::Float(n) => interm.push(ArithmeticTerm::Number(Number::Float(***n))),
        Literal::Rational(n) => interm.push(ArithmeticTerm::Number(Number::Rational(*n))),
        Literal::Atom(name) if name == &atom!("e") => interm.push(ArithmeticTerm::Number(
            Number::Float(OrderedFloat(f64::consts::E)),
        )),
        Literal::Atom(name) if name == &atom!("pi") => interm.push(ArithmeticTerm::Number(
            Number::Float(OrderedFloat(f64::consts::PI)),
        )),
        Literal::Atom(name) if name == &atom!("epsilon") => interm.push(ArithmeticTerm::Number(
            Number::Float(OrderedFloat(f64::EPSILON)),
        )),
        _ => return Err(ArithmeticError::NonEvaluableFunctor(*c, 0)),
    }

    Ok(())
}

impl<'a> ArithmeticEvaluator<'a> {
    pub(crate) fn new(bindings: &'a AllocVarDict, target_int: usize) -> Self {
        ArithmeticEvaluator {
            bindings,
            interm: Vec::new(),
            interm_c: target_int,
        }
    }

    fn get_unary_instr(
        &self,
        name: Atom,
        a1: ArithmeticTerm,
        t: usize,
    ) -> Result<Instruction, ArithmeticError> {
        match name {
            atom!("abs") => Ok(Instruction::Abs(a1, t)),
            atom!("-") => Ok(Instruction::Neg(a1, t)),
            atom!("+") => Ok(Instruction::Plus(a1, t)),
            atom!("cos") => Ok(Instruction::Cos(a1, t)),
            atom!("sin") => Ok(Instruction::Sin(a1, t)),
            atom!("tan") => Ok(Instruction::Tan(a1, t)),
            atom!("log") => Ok(Instruction::Log(a1, t)),
            atom!("exp") => Ok(Instruction::Exp(a1, t)),
            atom!("sqrt") => Ok(Instruction::Sqrt(a1, t)),
            atom!("acos") => Ok(Instruction::ACos(a1, t)),
            atom!("asin") => Ok(Instruction::ASin(a1, t)),
            atom!("atan") => Ok(Instruction::ATan(a1, t)),
            atom!("float") => Ok(Instruction::Float(a1, t)),
            atom!("truncate") => Ok(Instruction::Truncate(a1, t)),
            atom!("round") => Ok(Instruction::Round(a1, t)),
            atom!("ceiling") => Ok(Instruction::Ceiling(a1, t)),
            atom!("floor") => Ok(Instruction::Floor(a1, t)),
            atom!("sign") => Ok(Instruction::Sign(a1, t)),
            atom!("\\") => Ok(Instruction::BitwiseComplement(a1, t)),
            _ => Err(ArithmeticError::NonEvaluableFunctor(Literal::Atom(name), 1)),
        }
    }

    fn get_binary_instr(
        &self,
        name: Atom,
        a1: ArithmeticTerm,
        a2: ArithmeticTerm,
        t: usize,
    ) -> Result<Instruction, ArithmeticError> {
        match name {
            atom!("+") => Ok(Instruction::Add(a1, a2, t)),
            atom!("-") => Ok(Instruction::Sub(a1, a2, t)),
            atom!("/") => Ok(Instruction::Div(a1, a2, t)),
            atom!("//") => Ok(Instruction::IDiv(a1, a2, t)),
            atom!("max") => Ok(Instruction::Max(a1, a2, t)),
            atom!("min") => Ok(Instruction::Min(a1, a2, t)),
            atom!("div") => Ok(Instruction::IntFloorDiv(a1, a2, t)),
            atom!("rdiv") => Ok(Instruction::RDiv(a1, a2, t)),
            atom!("*") => Ok(Instruction::Mul(a1, a2, t)),
            atom!("**") => Ok(Instruction::Pow(a1, a2, t)),
            atom!("^") => Ok(Instruction::IntPow(a1, a2, t)),
            atom!(">>") => Ok(Instruction::Shr(a1, a2, t)),
            atom!("<<") => Ok(Instruction::Shl(a1, a2, t)),
            atom!("/\\") => Ok(Instruction::And(a1, a2, t)),
            atom!("\\/") => Ok(Instruction::Or(a1, a2, t)),
            atom!("xor") => Ok(Instruction::Xor(a1, a2, t)),
            atom!("mod") => Ok(Instruction::Mod(a1, a2, t)),
            atom!("rem") => Ok(Instruction::Rem(a1, a2, t)),
            atom!("gcd") => Ok(Instruction::Gcd(a1, a2, t)),
            atom!("atan2") => Ok(Instruction::ATan2(a1, a2, t)),
            _ => Err(ArithmeticError::NonEvaluableFunctor(Literal::Atom(name), 2)),
        }
    }

    fn incr_interm(&mut self) -> usize {
        let temp = self.interm_c;

        self.interm.push(ArithmeticTerm::Interm(temp));
        self.interm_c += 1;

        temp
    }

    fn instr_from_clause(
        &mut self,
        name: Atom,
        arity: usize,
    ) -> Result<Instruction, ArithmeticError> {
        match arity {
            1 => {
                let a1 = self.interm.pop().unwrap();

                let ninterm = if a1.interm_or(0) == 0 {
                    self.incr_interm()
                } else {
                    self.interm.push(a1.clone());
                    a1.interm_or(0)
                };

                self.get_unary_instr(name, a1, ninterm)
            }
            2 => {
                let a2 = self.interm.pop().unwrap();
                let a1 = self.interm.pop().unwrap();

                let min_interm = min(a1.interm_or(0), a2.interm_or(0));

                let ninterm = if min_interm == 0 {
                    let max_interm = max(a1.interm_or(0), a2.interm_or(0));

                    if max_interm == 0 {
                        self.incr_interm()
                    } else {
                        self.interm.push(ArithmeticTerm::Interm(max_interm));
                        self.interm_c = max_interm + 1;
                        max_interm
                    }
                } else {
                    self.interm.push(ArithmeticTerm::Interm(min_interm));
                    self.interm_c = min_interm + 1;
                    min_interm
                };

                self.get_binary_instr(name, a1, a2, ninterm)
            }
            _ => Err(ArithmeticError::NonEvaluableFunctor(
                Literal::Atom(name),
                arity,
            )),
        }
    }

    pub(crate) fn eval(&mut self, src: &'a Term) -> Result<ArithCont, ArithmeticError> {
        let mut code = vec![];
        let mut iter = src.iter()?;

        while let Some(term_ref) = iter.next() {
            match term_ref? {
                ArithTermRef::Literal(c) => push_literal(&mut self.interm, c)?,
                ArithTermRef::Var(cell, name) => {
                    let r = if cell.get().norm().reg_num() == 0 {
                        match self.bindings.get(&name) {
                            Some(&VarData::Temp(_, t, _)) if t != 0 => RegType::Temp(t),
                            Some(&VarData::Perm(p)) if p != 0 => RegType::Perm(p),
                            _ => return Err(ArithmeticError::UninstantiatedVar),
                        }
                    } else {
                        cell.get().norm()
                    };

                    self.interm.push(ArithmeticTerm::Reg(r));
                }
                ArithTermRef::Op(name, arity) => {
                    code.push(self.instr_from_clause(name, arity)?);
                }
            }
        }

        Ok((code, self.interm.pop()))
    }
}

// integer division rounding function -- 9.1.3.1.
pub(crate) fn rnd_i<'a>(n: &'a Number, arena: &mut Arena) -> Number {
    match n {
        &Number::Integer(_) | &Number::Fixnum(_) => *n,
        &Number::Float(OrderedFloat(f)) => fixnum!(Number, f.floor() as i64, arena),
        &Number::Rational(ref r) => {
            let r_ref = r.fract_floor_ref();
            let (mut fract, mut floor) = (Rational::new(), Integer::new());
            (&mut fract, &mut floor).assign(r_ref);

            Number::Integer(arena_alloc!(floor, arena))
        }
    }
}

impl From<Fixnum> for Integer {
    #[inline]
    fn from(n: Fixnum) -> Integer {
        Integer::from(n.get_num())
    }
}

// floating point rounding function -- 9.1.4.1.
pub(crate) fn rnd_f(n: &Number) -> f64 {
    match n {
        &Number::Fixnum(n) => n.get_num() as f64,
        &Number::Integer(ref n) => n.to_f64(),
        &Number::Float(OrderedFloat(f)) => f,
        &Number::Rational(ref r) => r.to_f64(),
    }
}

// floating point result function -- 9.1.4.2.
pub(crate) fn result_f<Round>(n: &Number, round: Round) -> Result<f64, EvalError>
where
    Round: Fn(&Number) -> f64,
{
    let f = rnd_f(n);
    classify_float(f, round)
}

fn classify_float<Round>(f: f64, round: Round) -> Result<f64, EvalError>
where
    Round: Fn(&Number) -> f64,
{
    match f.classify() {
        FpCategory::Normal | FpCategory::Zero => Ok(round(&Number::Float(OrderedFloat(f)))),
        FpCategory::Infinite => {
            let f = round(&Number::Float(OrderedFloat(f)));

            if OrderedFloat(f) == OrderedFloat(f64::MAX) {
                Ok(f)
            } else {
                Err(EvalError::FloatOverflow)
            }
        }
        FpCategory::Nan => Err(EvalError::Undefined),
        _ => Ok(round(&Number::Float(OrderedFloat(f)))),
    }
}

#[inline]
pub(crate) fn float_fn_to_f(n: i64) -> Result<f64, EvalError> {
    classify_float(n as f64, rnd_f)
}

#[inline]
pub(crate) fn float_i_to_f(n: &Integer) -> Result<f64, EvalError> {
    classify_float(n.to_f64(), rnd_f)
}

#[inline]
pub(crate) fn float_r_to_f(r: &Rational) -> Result<f64, EvalError> {
    classify_float(r.to_f64(), rnd_f)
}

#[inline]
pub(crate) fn add_f(f1: f64, f2: f64) -> Result<OrderedFloat<f64>, EvalError> {
    Ok(OrderedFloat(classify_float(f1 + f2, rnd_f)?))
}

#[inline]
pub(crate) fn mul_f(f1: f64, f2: f64) -> Result<OrderedFloat<f64>, EvalError> {
    Ok(OrderedFloat(classify_float(f1 * f2, rnd_f)?))
}

#[inline]
fn div_f(f1: f64, f2: f64) -> Result<OrderedFloat<f64>, EvalError> {
    if FpCategory::Zero == f2.classify() {
        Err(EvalError::ZeroDivisor)
    } else {
        Ok(OrderedFloat(classify_float(f1 / f2, rnd_f)?))
    }
}

impl Div<Number> for Number {
    type Output = Result<Number, EvalError>;

    fn div(self, rhs: Number) -> Self::Output {
        match (self, rhs) {
            (Number::Fixnum(n1), Number::Fixnum(n2)) => Ok(Number::Float(div_f(
                float_fn_to_f(n1.get_num())?,
                float_fn_to_f(n2.get_num())?,
            )?)),
            (Number::Fixnum(n1), Number::Integer(n2)) => Ok(Number::Float(div_f(
                float_fn_to_f(n1.get_num())?,
                float_i_to_f(&n2)?,
            )?)),
            (Number::Integer(n1), Number::Fixnum(n2)) => Ok(Number::Float(div_f(
                float_i_to_f(&n1)?,
                float_fn_to_f(n2.get_num())?,
            )?)),
            (Number::Fixnum(n1), Number::Rational(n2)) => Ok(Number::Float(div_f(
                float_fn_to_f(n1.get_num())?,
                float_r_to_f(&n2)?,
            )?)),
            (Number::Rational(n1), Number::Fixnum(n2)) => Ok(Number::Float(div_f(
                float_r_to_f(&n1)?,
                float_fn_to_f(n2.get_num())?,
            )?)),
            (Number::Fixnum(n1), Number::Float(OrderedFloat(n2))) => {
                Ok(Number::Float(div_f(float_fn_to_f(n1.get_num())?, n2)?))
            }
            (Number::Float(OrderedFloat(n1)), Number::Fixnum(n2)) => {
                Ok(Number::Float(div_f(n1, float_fn_to_f(n2.get_num())?)?))
            }
            (Number::Integer(n1), Number::Integer(n2)) => Ok(Number::Float(div_f(
                float_i_to_f(&n1)?,
                float_i_to_f(&n2)?,
            )?)),
            (Number::Integer(n1), Number::Float(OrderedFloat(n2))) => {
                Ok(Number::Float(div_f(float_i_to_f(&n1)?, n2)?))
            }
            (Number::Float(OrderedFloat(n2)), Number::Integer(n1)) => {
                Ok(Number::Float(div_f(n2, float_i_to_f(&n1)?)?))
            }
            (Number::Integer(n1), Number::Rational(n2)) => Ok(Number::Float(div_f(
                float_i_to_f(&n1)?,
                float_r_to_f(&n2)?,
            )?)),
            (Number::Rational(n2), Number::Integer(n1)) => Ok(Number::Float(div_f(
                float_r_to_f(&n2)?,
                float_i_to_f(&n1)?,
            )?)),
            (Number::Rational(n1), Number::Float(OrderedFloat(n2))) => {
                Ok(Number::Float(div_f(float_r_to_f(&n1)?, n2)?))
            }
            (Number::Float(OrderedFloat(n2)), Number::Rational(n1)) => {
                Ok(Number::Float(div_f(n2, float_r_to_f(&n1)?)?))
            }
            (Number::Float(OrderedFloat(f1)), Number::Float(OrderedFloat(f2))) => {
                Ok(Number::Float(div_f(f1, f2)?))
            }
            (Number::Rational(r1), Number::Rational(r2)) => Ok(Number::Float(div_f(
                float_r_to_f(&r1)?,
                float_r_to_f(&r2)?,
            )?)),
        }
    }
}

impl PartialEq for Number {
    fn eq(&self, rhs: &Self) -> bool {
        match (self, rhs) {
            (&Number::Fixnum(n1), &Number::Fixnum(n2)) => n1.eq(&n2),
            (&Number::Fixnum(n1), &Number::Integer(ref n2)) => n1.get_num().eq(&**n2),
            (&Number::Integer(ref n1), &Number::Fixnum(n2)) => (&**n1).eq(&n2.get_num()),
            (&Number::Fixnum(n1), &Number::Rational(ref n2)) => n1.get_num().eq(&**n2),
            (&Number::Rational(ref n1), &Number::Fixnum(n2)) => (&**n1).eq(&n2.get_num()),
            (&Number::Fixnum(n1), &Number::Float(n2)) => OrderedFloat(n1.get_num() as f64).eq(&n2),
            (&Number::Float(n1), &Number::Fixnum(n2)) => n1.eq(&OrderedFloat(n2.get_num() as f64)),
            (&Number::Integer(ref n1), &Number::Integer(ref n2)) => n1.eq(n2),
            (&Number::Integer(ref n1), Number::Float(n2)) => OrderedFloat(n1.to_f64()).eq(n2),
            (&Number::Float(n1), &Number::Integer(ref n2)) => n1.eq(&OrderedFloat(n2.to_f64())),
            (&Number::Integer(ref n1), &Number::Rational(ref n2)) => {
                #[cfg(feature = "num")]
                {
                    &Rational::from(&**n1) == &**n2
                }
                #[cfg(not(feature = "num"))]
                {
                    &**n1 == &**n2
                }
            }
            (&Number::Rational(ref n1), &Number::Integer(ref n2)) => {
                #[cfg(feature = "num")]
                {
                    &**n1 == &Rational::from(&**n2)
                }
                #[cfg(not(feature = "num"))]
                {
                    &**n1 == &**n2
                }
            }
            (&Number::Rational(ref n1), &Number::Float(n2)) => OrderedFloat(n1.to_f64()).eq(&n2),
            (&Number::Float(n1), &Number::Rational(ref n2)) => n1.eq(&OrderedFloat(n2.to_f64())),
            (&Number::Float(f1), &Number::Float(f2)) => f1.eq(&f2),
            (&Number::Rational(ref r1), &Number::Rational(ref r2)) => r1.eq(&r2),
        }
    }
}

impl Eq for Number {}

impl PartialOrd<usize> for Number {
    #[inline]
    fn partial_cmp(&self, rhs: &usize) -> Option<Ordering> {
        match self {
            Number::Fixnum(n) => {
                let n = n.get_num();

                if n < 0i64 {
                    Some(Ordering::Less)
                } else {
                    (n as usize).partial_cmp(rhs)
                }
            }
            Number::Integer(n) => (&**n).partial_cmp(rhs),
            Number::Rational(r) => (&**r).partial_cmp(rhs),
            Number::Float(f) => f.partial_cmp(&OrderedFloat(*rhs as f64)),
        }
    }
}

impl PartialEq<usize> for Number {
    #[inline]
    fn eq(&self, rhs: &usize) -> bool {
        match self {
            Number::Fixnum(n) => {
                let n = n.get_num();

                if n < 0i64 {
                    false
                } else {
                    (n as usize).eq(rhs)
                }
            }
            Number::Integer(n) => (&**n).eq(rhs),
            Number::Rational(r) => (&**r).eq(rhs),
            Number::Float(f) => f.eq(&OrderedFloat(*rhs as f64)),
        }
    }
}

impl PartialOrd for Number {
    fn partial_cmp(&self, rhs: &Number) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}

impl Ord for Number {
    fn cmp(&self, rhs: &Number) -> Ordering {
        match (self, rhs) {
            (&Number::Fixnum(n1), &Number::Fixnum(n2)) => n1.get_num().cmp(&n2.get_num()),
            (&Number::Fixnum(n1), Number::Integer(n2)) => Integer::from(n1.get_num()).cmp(&*n2),
            (Number::Integer(n1), &Number::Fixnum(n2)) => (&**n1).cmp(&Integer::from(n2.get_num())),
            (&Number::Fixnum(n1), Number::Rational(n2)) => Rational::from(n1.get_num()).cmp(&*n2),
            (Number::Rational(n1), &Number::Fixnum(n2)) => {
                (&**n1).cmp(&Rational::from(n2.get_num()))
            }
            (&Number::Fixnum(n1), &Number::Float(n2)) => OrderedFloat(n1.get_num() as f64).cmp(&n2),
            (&Number::Float(n1), &Number::Fixnum(n2)) => n1.cmp(&OrderedFloat(n2.get_num() as f64)),
            (&Number::Integer(n1), &Number::Integer(n2)) => (*n1).cmp(&*n2),
            (&Number::Integer(n1), Number::Float(n2)) => OrderedFloat(n1.to_f64()).cmp(n2),
            (&Number::Float(n1), &Number::Integer(ref n2)) => n1.cmp(&OrderedFloat(n2.to_f64())),
            (&Number::Integer(n1), &Number::Rational(n2)) => {
                #[cfg(feature = "num")]
                {
                    Rational::from(&**n1).cmp(n2)
                }
                #[cfg(not(feature = "num"))]
                {
                    (&*n1).partial_cmp(&*n2).unwrap_or(Ordering::Less)
                }
            }
            (&Number::Rational(n1), &Number::Integer(n2)) => {
                #[cfg(feature = "num")]
                {
                    (&**n1).cmp(&Rational::from(&**n2))
                }
                #[cfg(not(feature = "num"))]
                {
                    (&*n1).partial_cmp(&*n2).unwrap_or(Ordering::Less)
                }
            }
            (&Number::Rational(n1), &Number::Float(n2)) => OrderedFloat(n1.to_f64()).cmp(&n2),
            (&Number::Float(n1), &Number::Rational(n2)) => n1.cmp(&OrderedFloat(n2.to_f64())),
            (&Number::Float(f1), &Number::Float(f2)) => f1.cmp(&f2),
            (&Number::Rational(r1), &Number::Rational(r2)) => (*r1).cmp(&*r2),
        }
    }
}

impl TryFrom<HeapCellValue> for Number {
    type Error = ();

    #[inline]
    fn try_from(value: HeapCellValue) -> Result<Number, Self::Error> {
        read_heap_cell!(value,
           (HeapCellValueTag::Cons, c) => {
               match_untyped_arena_ptr!(c,
                  (ArenaHeaderTag::F64, n) => {
                      Ok(Number::Float(*n))
                  }
                  (ArenaHeaderTag::Integer, n) => {
                      Ok(Number::Integer(n))
                  }
                  (ArenaHeaderTag::Rational, n) => {
                      Ok(Number::Rational(n))
                  }
                  _ => {
                      Err(())
                  }
               )
           }
           (HeapCellValueTag::F64, n) => {
               Ok(Number::Float(**n))
           }
           (HeapCellValueTag::Fixnum, n) => {
               Ok(Number::Fixnum(n))
           }
           _ => {
               Err(())
           }
        )
    }
}

// Computes n ^ power. Ignores the sign of power.
pub(crate) fn binary_pow(mut n: Integer, power: &Integer) -> Integer {
    let mut power = Integer::from(power.abs_ref());

    if power == 0 {
        return Integer::from(1);
    }

    let mut oddand = Integer::from(1);

    while power > 1 {
        if power.is_odd() {
            oddand *= &n;
        }

        n.pow_assign(2);
        power >>= 1;
    }

    n * oddand
}

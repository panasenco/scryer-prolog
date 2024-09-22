#![allow(clippy::new_without_default)] // annotating structs annotated with #[bitfield] doesn't work

use crate::arena::*;
use crate::atom_table::*;
use crate::forms::PredicateKey;
use crate::machine::heap::*;
use crate::machine::machine_indices::*;
use crate::types::*;

use std::fmt;
use std::hash::Hash;
use std::io::{Error as IOError, ErrorKind};
use std::ops::Neg;
use std::rc::Rc;
use std::vec::Vec;

use fxhash::FxBuildHasher;
use indexmap::IndexMap;
use scryer_modular_bitfield::error::OutOfBounds;
use scryer_modular_bitfield::prelude::*;

pub type Specifier = u32;

pub const MAX_ARITY: usize = 255;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum OpDeclSpec {
    XFX = 0x0001,
    XFY = 0x0002,
    YFX = 0x0004,
    XF = 0x0010,
    YF = 0x0020,
    FX = 0x0040,
    FY = 0x0080,
}

pub use OpDeclSpec::*;

impl OpDeclSpec {
    pub const fn value(self) -> u32 {
        self as u32
    }

    pub fn get_spec(self) -> Atom {
        match self {
            XFX => atom!("xfx"),
            XFY => atom!("xfy"),
            YFX => atom!("yfx"),
            FX => atom!("fx"),
            FY => atom!("fy"),
            XF => atom!("xf"),
            YF => atom!("yf"),
        }
    }

    pub const fn is_prefix(self) -> bool {
        matches!(self, Self::FX | Self::FY)
    }

    pub const fn is_postfix(self) -> bool {
        matches!(self, Self::XF | Self::YF)
    }

    pub const fn is_infix(self) -> bool {
        matches!(self, Self::XFX | Self::XFY | Self::YFX)
    }

    pub const fn is_strict_left(self) -> bool {
        matches!(self, Self::XFX | Self::XFY | Self::XF)
    }

    pub const fn is_strict_right(self) -> bool {
        matches!(self, Self::XFX | Self::YFX | Self::FX)
    }

    #[inline(always)]
    pub(crate) fn fixity(self) -> Fixity {
        match self {
            XFY | XFX | YFX => Fixity::In,
            XF | YF => Fixity::Post,
            FX | FY => Fixity::Pre,
        }
    }
}

impl From<OpDeclSpec> for u8 {
    fn from(value: OpDeclSpec) -> Self {
        value as u8
    }
}

impl From<OpDeclSpec> for u32 {
    fn from(value: OpDeclSpec) -> Self {
        value as u32
    }
}

impl TryFrom<u8> for OpDeclSpec {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0x0001 => XFX,
            0x0002 => XFY,
            0x0004 => YFX,
            0x0010 => XF,
            0x0020 => YF,
            0x0040 => FX,
            0x0080 => FY,
            _ => return Err(()),
        })
    }
}

impl TryFrom<Atom> for OpDeclSpec {
    type Error = ();

    fn try_from(value: Atom) -> Result<Self, Self::Error> {
        Ok(match value {
            atom!("xfx") => Self::XFX,
            atom!("xfy") => Self::XFY,
            atom!("yfx") => Self::YFX,
            atom!("fx") => Self::FX,
            atom!("fy") => Self::FY,
            atom!("xf") => Self::XF,
            atom!("yf") => Self::YF,
            _ => return Err(()),
        })
    }
}

pub const DELIMITER: u32 = 0x0100;
pub const TERM: u32 = 0x1000;
pub const LTERM: u32 = 0x3000;
pub const BTERM: u32 = 0x11000;

pub const NEGATIVE_SIGN: u32 = 0x0200;

#[macro_export]
macro_rules! fixnum {
    ($n:expr, $arena:expr) => {
        Fixnum::build_with_checked($n)
            .map(|n| fixnum_as_cell!(n))
            .unwrap_or_else(|_| typed_arena_ptr_as_cell!(arena_alloc!(Integer::from($n), $arena) as TypedArenaPtr<Integer>))
    };
    ($wrapper:ty, $n:expr, $arena:expr) => {
        Fixnum::build_with_checked($n)
            .map(<$wrapper>::Fixnum)
            .unwrap_or_else(|_| <$wrapper>::Integer(arena_alloc!(Integer::from($n), $arena)))
    };
}

macro_rules! is_term {
    ($x:expr) => {
        ($x as u32 & $crate::parser::ast::TERM) != 0 || is_negate!($x)
    };
}

macro_rules! is_lterm {
    ($x:expr) => {
        ($x as u32 & $crate::parser::ast::LTERM) != 0 || is_negate!($x)
    };
}

macro_rules! is_op {
    ($x:expr) => {
        $x as u32
            & ($crate::parser::ast::XF as u32
                | $crate::parser::ast::YF as u32
                | $crate::parser::ast::FX as u32
                | $crate::parser::ast::FY as u32
                | $crate::parser::ast::XFX as u32
                | $crate::parser::ast::XFY as u32
                | $crate::parser::ast::YFX as u32)
            != 0
    };
}

macro_rules! is_negate {
    ($x:expr) => {
        ($x as u32 & $crate::parser::ast::NEGATIVE_SIGN) != 0
    };
}

#[macro_export]
macro_rules! is_prefix {
    ($x:expr) => {
        $x as u32 & ($crate::parser::ast::FX as u32 | $crate::parser::ast::FY as u32) != 0
    };
}

#[macro_export]
macro_rules! is_postfix {
    ($x:expr) => {
        $x as u32 & ($crate::parser::ast::XF as u32 | $crate::parser::ast::YF as u32) != 0
    };
}

#[macro_export]
macro_rules! is_infix {
    ($x:expr) => {
        ($x as u32
            & ($crate::parser::ast::XFX as u32
                | $crate::parser::ast::XFY as u32
                | $crate::parser::ast::YFX as u32))
            != 0
    };
}

#[macro_export]
macro_rules! is_xfx {
    ($x:expr) => {
        ($x as u32 & $crate::parser::ast::XFX as u32) != 0
    };
}

#[macro_export]
macro_rules! is_xfy {
    ($x:expr) => {
        ($x as u32 & $crate::parser::ast::XFY as u32) != 0
    };
}

#[macro_export]
macro_rules! is_yfx {
    ($x:expr) => {
        ($x as u32 & $crate::parser::ast::YFX as u32) != 0
    };
}
#[macro_export]
macro_rules! is_yf {
    ($x:expr) => {
        ($x as u32 & $crate::parser::ast::YF as u32) != 0
    };
}

#[macro_export]
macro_rules! is_xf {
    ($x:expr) => {
        ($x as u32 & $crate::parser::ast::XF as u32) != 0
    };
}

#[macro_export]
macro_rules! is_fx {
    ($x:expr) => {
        ($x as u32 & $crate::parser::ast::FX as u32) != 0
    };
}

#[macro_export]
macro_rules! is_fy {
    ($x:expr) => {
        ($x as u32 & $crate::parser::ast::FY as u32) != 0
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegType {
    Perm(usize),
    Temp(usize),
}

impl Default for RegType {
    fn default() -> Self {
        RegType::Temp(0)
    }
}

impl RegType {
    pub fn reg_num(self) -> usize {
        match self {
            RegType::Perm(reg_num) | RegType::Temp(reg_num) => reg_num,
        }
    }

    pub fn is_perm(self) -> bool {
        matches!(self, RegType::Perm(_))
    }
}

impl fmt::Display for RegType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RegType::Perm(val) => write!(f, "Y{}", val),
            RegType::Temp(val) => write!(f, "X{}", val),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum VarReg {
    ArgAndNorm(RegType, usize),
    Norm(RegType),
}

impl VarReg {
    pub fn norm(self) -> RegType {
        match self {
            VarReg::ArgAndNorm(reg, _) | VarReg::Norm(reg) => reg,
        }
    }
}

impl fmt::Display for VarReg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            VarReg::Norm(RegType::Perm(reg)) => write!(f, "Y{}", reg),
            VarReg::Norm(RegType::Temp(reg)) => write!(f, "X{}", reg),
            VarReg::ArgAndNorm(RegType::Perm(reg), arg) => write!(f, "Y{} A{}", reg, arg),
            VarReg::ArgAndNorm(RegType::Temp(reg), arg) => write!(f, "X{} A{}", reg, arg),
        }
    }
}

impl Default for VarReg {
    fn default() -> Self {
        VarReg::Norm(RegType::default())
    }
}

#[macro_export]
macro_rules! temp_v {
    ($x:expr) => {
        $crate::parser::ast::RegType::Temp($x)
    };
}

#[macro_export]
macro_rules! perm_v {
    ($x:expr) => {
        $crate::parser::ast::RegType::Perm($x)
    };
}

#[bitfield]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub struct OpDesc {
    prec: B11,
    spec: B8,
    #[allow(unused)]
    padding: B13,
}

impl OpDesc {
    #[inline]
    pub fn build_with(prec: u16, spec: OpDeclSpec) -> Self {
        OpDesc::new().with_spec(spec as u8).with_prec(prec)
    }

    #[inline]
    pub fn get(self) -> (u16, OpDeclSpec) {
        (self.prec(), self.get_spec())
    }

    pub fn set(&mut self, prec: u16, spec: OpDeclSpec) {
        self.set_prec(prec);
        self.set_spec(spec as u8);
    }

    #[inline]
    pub fn get_prec(self) -> u16 {
        self.prec()
    }

    #[inline]
    pub fn get_spec(self) -> OpDeclSpec {
        OpDeclSpec::try_from(self.spec()).expect("OpDecl always contains a valud OpDeclSpec")
    }

    #[inline]
    pub fn arity(self) -> usize {
        if !self.get_spec().is_infix() {
            1
        } else {
            2
        }
    }
}

// name and fixity -> operator type and precedence.
pub type OpDir = IndexMap<(Atom, Fixity), OpDesc, FxBuildHasher>;

#[derive(Debug, Default, Clone, Copy)]
pub struct MachineFlags {
    pub double_quotes: DoubleQuotes,
    pub unknown: Unknown,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum DoubleQuotes {
    Atom,
    #[default]
    Chars,
    Codes,
}

impl DoubleQuotes {
    pub fn is_chars(self) -> bool {
        matches!(self, DoubleQuotes::Chars)
    }

    pub fn is_atom(self) -> bool {
        matches!(self, DoubleQuotes::Atom)
    }

    pub fn is_codes(self) -> bool {
        matches!(self, DoubleQuotes::Codes)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub enum Unknown {
    #[default]
    Error,
    Fail,
    Warn,
}

impl Unknown {
    pub fn is_error(self) -> bool {
        matches!(self, Unknown::Error)
    }

    pub fn is_fail(self) -> bool {
        matches!(self, Unknown::Fail)
    }

    pub fn is_warn(self) -> bool {
        matches!(self, Unknown::Warn)
    }
}

pub fn default_op_dir() -> OpDir {
    let mut op_dir = OpDir::with_hasher(FxBuildHasher::default());

    op_dir.insert((atom!(":-"), Fixity::In), OpDesc::build_with(1200, XFX));
    op_dir.insert((atom!(":-"), Fixity::Pre), OpDesc::build_with(1200, FX));
    op_dir.insert((atom!("?-"), Fixity::Pre), OpDesc::build_with(1200, FX));
    op_dir.insert((atom!(","), Fixity::In), OpDesc::build_with(1000, XFY));

    op_dir
}

#[derive(Debug, Clone)]
pub enum ArithmeticError {
    NonEvaluableFunctor(HeapCellValue, usize),
    UninstantiatedVar,
}

#[derive(Debug, Copy, Clone, Default)]
pub struct ParserErrorSrc {
    pub col_num: usize,
    pub line_num: usize,
}

#[derive(Debug)]
pub enum ParserError {
    BackQuotedString(ParserErrorSrc),
    IO(IOError, ParserErrorSrc),
    IncompleteReduction(ParserErrorSrc),
    InvalidSingleQuotedCharacter(char, ParserErrorSrc),
    LexicalError(lexical::Error, ParserErrorSrc),
    MissingQuote(ParserErrorSrc),
    NonPrologChar(ParserErrorSrc),
    ParseBigInt(ParserErrorSrc),
    ResourceError(usize, ParserErrorSrc),
    UnexpectedChar(char, ParserErrorSrc),
    // UnexpectedEOF,
    Utf8Error(ParserErrorSrc),
}

impl ParserError {
    pub fn err_src(&self) -> ParserErrorSrc {
        match self {
            &ParserError::BackQuotedString(err_src)
            | &ParserError::IO(_, err_src)
            | &ParserError::IncompleteReduction(err_src)
            | &ParserError::InvalidSingleQuotedCharacter(_, err_src)
            | &ParserError::LexicalError(_, err_src)
            | &ParserError::MissingQuote(err_src)
            | &ParserError::NonPrologChar(err_src)
            | &ParserError::ParseBigInt(err_src)
            | &ParserError::ResourceError(_, err_src)
            | &ParserError::UnexpectedChar(_, err_src)
            | &ParserError::Utf8Error(err_src) => err_src,
        }
    }

    pub fn as_atom(&self) -> Atom {
        match self {
            ParserError::BackQuotedString(..) => atom!("back_quoted_string"),
            ParserError::IncompleteReduction(..) => atom!("incomplete_reduction"),
            ParserError::InvalidSingleQuotedCharacter(..) => {
                atom!("invalid_single_quoted_character")
            }
            ParserError::IO(e, _) if e.kind() == ErrorKind::UnexpectedEof => {
                atom!("unexpected_end_of_file")
            }
            ParserError::IO(e, _) if e.kind() == ErrorKind::InvalidData => {
                atom!("invalid_data")
            }
            ParserError::IO(..) => atom!("input_output_error"),
            ParserError::LexicalError(..) => atom!("lexical_error"),
            ParserError::MissingQuote(..) => atom!("missing_quote"),
            ParserError::NonPrologChar(..) => atom!("non_prolog_character"),
            ParserError::ParseBigInt(..) => atom!("cannot_parse_big_int"),
            ParserError::UnexpectedChar(..) => atom!("unexpected_char"),
            ParserError::Utf8Error(..) => atom!("utf8_conversion_error"),
            ParserError::ResourceError(..) => atom!("resource_error"),
        }
    }

    #[inline]
    pub fn unexpected_eof(err_src: ParserErrorSrc) -> Self {
        ParserError::IO(std::io::Error::from(ErrorKind::UnexpectedEof), err_src)
    }

    #[inline]
    pub fn is_unexpected_eof(&self) -> bool {
        if let ParserError::IO(e, _) = self {
            e.kind() == ErrorKind::UnexpectedEof
        } else {
            false
        }
    }
}

impl From<(lexical::Error, ParserErrorSrc)> for ParserError {
    fn from((e, err_src): (lexical::Error, ParserErrorSrc)) -> ParserError {
        ParserError::LexicalError(e, err_src)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CompositeOpDir<'a, 'b> {
    pub primary_op_dir: Option<&'b OpDir>,
    pub secondary_op_dir: &'a OpDir,
}

impl<'a, 'b> CompositeOpDir<'a, 'b> {
    #[inline]
    pub fn new(secondary_op_dir: &'a OpDir, primary_op_dir: Option<&'b OpDir>) -> Self {
        CompositeOpDir {
            primary_op_dir,
            secondary_op_dir,
        }
    }

    #[inline]
    pub(crate) fn get(&self, name: Atom, fixity: Fixity) -> Option<OpDesc> {
        let entry = if let Some(primary_op_dir) = &self.primary_op_dir {
            primary_op_dir.get(&(name, fixity))
        } else {
            None
        };

        entry
            .or_else(move || self.secondary_op_dir.get(&(name, fixity)))
            .cloned()
    }
}

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub enum Fixity {
    In,
    Post,
    Pre,
}

#[bitfield]
#[repr(u64)]
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct Fixnum {
    num: B56,
    #[allow(unused)]
    f: bool,
    #[allow(unused)]
    m: bool,
    #[allow(unused)]
    tag: B6,
}

impl Fixnum {
    #[inline]
    pub fn build_with(num: i64) -> Self {
        Fixnum::new()
            .with_num(u64::from_ne_bytes(num.to_ne_bytes()) & ((1 << 56) - 1))
            .with_tag(HeapCellValueTag::Fixnum as u8)
            .with_m(false)
            .with_f(false)
    }

    #[inline]
    pub fn as_cutpoint(num: i64) -> Self {
        Fixnum::new()
            .with_num(u64::from_ne_bytes(num.to_ne_bytes()) & ((1 << 56) - 1))
            .with_tag(HeapCellValueTag::CutPoint as u8)
            .with_m(false)
            .with_f(false)
    }

    #[inline]
    pub fn get_tag(&self) -> HeapCellValueTag {
        use scryer_modular_bitfield::Specifier;
        HeapCellValueTag::from_bytes(self.tag()).unwrap()
    }

    #[inline]
    pub fn build_with_checked(num: i64) -> Result<Self, OutOfBounds> {
        const UPPER_BOUND: i64 = (1 << 55) - 1;
        const LOWER_BOUND: i64 = -(1 << 55);

        if (LOWER_BOUND..=UPPER_BOUND).contains(&num) {
            Ok(Fixnum::new()
                .with_m(false)
                .with_f(false)
                .with_tag(HeapCellValueTag::Fixnum as u8)
                .with_num(u64::from_ne_bytes(num.to_ne_bytes()) & ((1 << 56) - 1)))
        } else {
            Err(OutOfBounds {})
        }
    }

    #[inline]
    pub fn get_num(self) -> i64 {
        let n = self.num() as i64;
        let (n, overflowed) = (n << 8).overflowing_shr(8);
        debug_assert!(!overflowed);
        n
    }
}

impl Neg for Fixnum {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self::Output {
        Fixnum::build_with(-self.get_num())
    }
}

pub type Var = Rc<String>;

pub(crate) fn subterm_index(
    heap: &impl SizedHeap,
    subterm_loc: usize,
) -> (usize, HeapCellValue) {
    let subterm = heap[subterm_loc];

    if subterm.is_ref() {
        let subterm = heap_bound_deref(heap, subterm);
        let subterm_loc = subterm.get_value() as usize;
        let subterm = heap_bound_store(heap, subterm);

        let subterm_loc = if subterm.is_ref() {
            subterm.get_value() as usize
        } else {
            subterm_loc
        };

        (subterm_loc, subterm)
    } else {
        (subterm_loc, subterm)
    }
}

pub(crate) fn fetch_index_ptr(
    heap: &impl SizedHeap,
    arity: usize,
    term_loc: usize,
) -> Option<CodeIndex> {
    if term_loc + arity + 1 >= heap.cell_len() || heap.pstr_at(term_loc + arity + 1) {
        return None;
    }

    read_heap_cell!(heap[term_loc + arity + 1],
        (HeapCellValueTag::Cons, c) => {
            match_untyped_arena_ptr!(c,
               (ArenaHeaderTag::IndexPtr, ptr) => {
                   return Some(CodeIndex::from(ptr));
               }
               _ => {}
            );
        }
        _ => {}
    );

    None
}

pub(crate) fn blunt_index_ptr(
    heap: &mut impl SizedHeapMut,
    key: PredicateKey,
    term_loc: usize,
) -> bool {
    if fetch_index_ptr(heap, key.1, term_loc).is_some() {
        heap[term_loc] = atom_as_cell!(key.0, key.1);
        true
    } else {
        false
    }
}

pub(crate) fn unfold_by_str_once(
    heap: &mut impl SizedHeapMut,
    start_term: HeapCellValue,
    atom: Atom,
) -> Option<usize> {
    let start_term = heap_bound_store(
        heap,
        heap_bound_deref(heap, start_term),
    );

    if let HeapCellValueTag::Str = start_term.get_tag() {
        let s = start_term.get_value() as usize;

        let (s_atom, s_arity) = cell_as_atom_cell!(heap[s]).get_name_and_arity();
        blunt_index_ptr(heap, (s_atom, s_arity), s);

        if (s_atom, s_arity) == (atom, 2) {
            return Some(s+1);
        }
    }

    None
}

pub fn unfold_by_str(
    heap: &mut impl SizedHeapMut,
    mut start_term: HeapCellValue,
    atom: Atom,
) -> Vec<HeapCellValue> {
    let mut terms = vec![];
    start_term = heap_bound_store(heap, heap_bound_deref(heap, start_term));

    while let Some(fst_loc) = unfold_by_str_once(heap, start_term, atom) {
        let (_, snd) = subterm_index(heap, fst_loc + 1);
        let (_, fst) = subterm_index(heap, fst_loc);
        terms.push(fst);
        start_term = snd;
    }

    terms
}

pub fn unfold_by_str_locs(
    heap: &mut impl SizedHeapMut,
    mut term_loc: usize,
    atom: Atom,
) -> Vec<(HeapCellValue, usize)> {
    let mut terms = vec![];
    let mut current_term = heap[term_loc];

    while let Some(fst_loc) = unfold_by_str_once(heap, current_term, atom) {
        term_loc = fst_loc+1;
        current_term = heap[term_loc];
        let fst = heap[fst_loc];
        terms.push((fst, fst_loc));
    }

    terms.push((current_term, term_loc));
    terms
}

pub fn term_predicate_key_from_heap(
    heap: &impl SizedHeap,
    value: HeapCellValue,
) -> Option<PredicateKey> {
    read_heap_cell!(value,
       (HeapCellValueTag::Atom, (name, _arity)) => {
           debug_assert_eq!(_arity, 0);
           Some((name, 0))
       }
       _ => {
           term_predicate_key(heap, value.get_value() as usize)
       }
    )
}

pub fn term_predicate_key(
    heap: &impl SizedHeap,
    mut term_loc: usize,
) -> Option<PredicateKey> {
    loop {
        read_heap_cell!(heap[term_loc],
            (HeapCellValueTag::Atom, (name, arity)) => {
                return Some((name, arity));
            }
            (HeapCellValueTag::Str, s) => {
                term_loc = s;
            }
            (HeapCellValueTag::AttrVar | HeapCellValueTag::Var, h) => {
                if h != term_loc {
                    term_loc = h;
                } else {
                    return None;
                }
            }
            _ => {
                return None;
            }
        );
    }
}

pub fn inverse_var_locs_from_iter<I: Iterator<Item = HeapCellValue>>(iter: I) -> InverseVarLocs {
    let mut occurrence_set: IndexMap<HeapCellValue, usize, FxBuildHasher> =
        IndexMap::with_hasher(FxBuildHasher::default());

    for term in iter {
        if term.is_var() {
            let var_count = occurrence_set.entry(term).or_insert(0);
            *var_count += 1;
        }
    }

    let mut inverse_var_locs = InverseVarLocs::default();

    for (var, count) in occurrence_set {
        let var_loc = var.get_value() as usize;

        if count > 1 {
            inverse_var_locs.insert(
                var_loc,
                Rc::new(format!("_{}", var_loc)),
            );
        }
    }

    inverse_var_locs
}

pub fn term_nth_arg(heap: &impl SizedHeap, mut term_loc: usize, n: usize) -> Option<usize> {
    loop {
        read_heap_cell!(heap[term_loc],
            (HeapCellValueTag::Str, s) => {
                return if cell_as_atom_cell!(heap[s]).get_arity() >= n {
                    Some(s+n)
                } else {
                    None
                };
            }
            (HeapCellValueTag::Atom, (_name, arity)) => {
                return if arity >= n {
                    Some(term_loc + n)
                } else {
                    None
                };
            }
            (HeapCellValueTag::Lis, l) => {
                return if 1 <= n && n <= 2 {
                    Some(l+n-1)
                } else if n == 0 {
                    Some(term_loc)
                } else {
                    None
                };
            }
            (HeapCellValueTag::AttrVar | HeapCellValueTag::Var, h) => {
                if h != term_loc {
                    term_loc = h;
                } else {
                    return None;
                }
            }
            _ => {
                return None;
            }
        );
    }
}

#[derive(Debug)]
pub struct TermWriteResult {
    pub focus: usize,
    pub inverse_var_locs: InverseVarLocs,
}

pub type VarLocs = IndexMap<Var, HeapCellValue, FxBuildHasher>;
pub type InverseVarLocs = IndexMap<usize, Var, FxBuildHasher>;

#[derive(Debug)]
pub struct FocusedHeap<'a> {
    pub heap: &'a mut Heap,
    pub focus: usize,
    pub inverse_var_locs: InverseVarLocs,
}

impl<'a> FocusedHeap<'a> {
    #[inline]
    pub fn from(heap: &'a mut Heap, focus: usize, inverse_var_locs: InverseVarLocs) -> Self {
        Self { heap, focus, inverse_var_locs }
    }

    pub fn deref_loc(&self, term_loc: usize) -> HeapCellValue {
        use crate::machine::heap::*;

        let cell = self.heap[term_loc];
        heap_bound_store(self.heap, heap_bound_deref(self.heap, cell))
    }

    pub fn predicate_key(&self, term_loc: usize) -> Option<PredicateKey> {
        term_predicate_key(self.heap, term_loc)
    }

    pub fn nth_arg(&self, term_loc: usize, n: usize) -> Option<usize> {
        term_nth_arg(self.heap, term_loc, n)
    }
}

#[derive(Debug)]
pub struct FocusedHeapRefMut<'a> {
    pub heap: &'a mut Heap,
    pub focus: usize,
}

impl<'a> FocusedHeapRefMut<'a> {
    #[inline]
    pub fn from(heap: &'a mut Heap, focus: usize) -> Self {
        Self { heap, focus }
    }

    pub fn predicate_key(&self, term_loc: usize) -> Option<PredicateKey> {
        term_predicate_key(self.heap, term_loc)
    }

    pub fn arity(&self, term_loc: usize) -> usize {
        self.predicate_key(term_loc)
            .map(|(_, arity)| arity)
            .unwrap_or(0)
    }

    pub fn deref_loc(&self, term_loc: usize) -> HeapCellValue {
        let cell = self.heap[term_loc];
        heap_bound_store(self.heap, heap_bound_deref(self.heap, cell))
    }

    pub fn nth_arg(&self, term_loc: usize, n: usize) -> Option<usize> {
        term_nth_arg(self.heap, term_loc, n)
    }
}

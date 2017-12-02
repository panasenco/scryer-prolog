use prolog::ast::*;
use prolog::codegen::*;
use prolog::debray_allocator::*;
use prolog::machine::*;

use termion::raw::IntoRawMode;
use termion::input::TermRead;
use termion::event::Key;

use std::io::{Write, stdin, stdout};
use std::fmt;

impl fmt::Display for FactInstruction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &FactInstruction::GetConstant(Level::Shallow, ref constant, ref r) =>
                write!(f, "get_constant {}, A{}", constant, r.reg_num()),
            &FactInstruction::GetConstant(Level::Deep, ref constant, ref r) =>
                write!(f, "get_constant {}, {}", constant, r),
            &FactInstruction::GetList(Level::Shallow, ref r) =>
                write!(f, "get_list A{}", r.reg_num()),
            &FactInstruction::GetList(Level::Deep, ref r) =>
                write!(f, "get_list {}", r),
            &FactInstruction::GetStructure(Level::Deep, ref name, ref arity, ref r) =>
                write!(f, "get_structure {}/{}, {}", name, arity, r),
            &FactInstruction::GetStructure(Level::Shallow, ref name, ref arity, ref r) =>
                write!(f, "get_structure {}/{}, A{}", name, arity, r.reg_num()),
            &FactInstruction::GetValue(ref x, ref a) =>
                write!(f, "get_value {}, A{}", x, a),
            &FactInstruction::GetVariable(ref x, ref a) =>
                write!(f, "fact:get_variable {}, A{}", x, a),
            &FactInstruction::UnifyConstant(ref constant) =>
                write!(f, "unify_constant {}", constant),
            &FactInstruction::UnifyVariable(ref r) =>
                write!(f, "unify_variable {}", r),
            &FactInstruction::UnifyLocalValue(ref r) =>
                write!(f, "unify_local_value {}", r),
            &FactInstruction::UnifyValue(ref r) =>
                write!(f, "unify_value {}", r),
            &FactInstruction::UnifyVoid(n) =>
                write!(f, "unify_void {}", n)
        }
    }
}

impl fmt::Display for QueryInstruction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &QueryInstruction::GetVariable(ref x, ref a) =>
                write!(f, "query:get_variable {}, A{}", x, a),
            &QueryInstruction::PutConstant(Level::Shallow, ref constant, ref r) =>
                write!(f, "put_constant {}, A{}", constant, r.reg_num()),
            &QueryInstruction::PutConstant(Level::Deep, ref constant, ref r) =>
                write!(f, "put_constant {}, {}", constant, r),
            &QueryInstruction::PutList(Level::Shallow, ref r) =>
                write!(f, "put_list A{}", r.reg_num()),
            &QueryInstruction::PutList(Level::Deep, ref r) =>
                write!(f, "put_list {}", r),
            &QueryInstruction::PutStructure(Level::Deep, ref name, ref arity, ref r) =>
                write!(f, "put_structure {}/{}, {}", name, arity, r),
            &QueryInstruction::PutStructure(Level::Shallow, ref name, ref arity, ref r) =>
                write!(f, "put_structure {}/{}, A{}", name, arity, r.reg_num()),
            &QueryInstruction::PutUnsafeValue(y, a) =>
                write!(f, "put_unsafe_value Y{}, A{}", y, a),
            &QueryInstruction::PutValue(ref x, ref a) =>
                write!(f, "put_value {}, A{}", x, a),
            &QueryInstruction::PutVariable(ref x, ref a) =>
                write!(f, "put_variable {}, A{}", x, a),
            &QueryInstruction::SetConstant(ref constant) =>
                write!(f, "set_constant {}", constant),
            &QueryInstruction::SetLocalValue(ref r) =>
                write!(f, "set_local_value {}", r),
            &QueryInstruction::SetVariable(ref r) =>
                write!(f, "set_variable {}", r),
            &QueryInstruction::SetValue(ref r) =>
                write!(f, "set_value {}", r),
            &QueryInstruction::SetVoid(n) =>
                write!(f, "set_void {}", n)
        }
    }
}

impl fmt::Display for ControlInstruction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &ControlInstruction::Allocate(num_cells) =>
                write!(f, "allocate {}", num_cells),
            &ControlInstruction::Call(ref name, arity, pvs) =>
                write!(f, "call {}/{}, {}", name, arity, pvs),
            &ControlInstruction::CallN(arity) =>
                write!(f, "call_N {}", arity),
            &ControlInstruction::CatchCall =>
                write!(f, "call_catch"),
            &ControlInstruction::CatchExecute =>
                write!(f, "execute_catch"),
            &ControlInstruction::ExecuteN(arity) =>
                write!(f, "execute_N {}", arity),
            &ControlInstruction::Deallocate =>
                write!(f, "deallocate"),
            &ControlInstruction::Execute(ref name, arity) =>
                write!(f, "execute {}/{}", name, arity),
            &ControlInstruction::Goto(p, arity) =>
                write!(f, "goto {}/{}", p, arity),
            &ControlInstruction::Proceed =>
                write!(f, "proceed"),
            &ControlInstruction::ThrowCall =>
                write!(f, "call_throw"),
            &ControlInstruction::ThrowExecute =>
                write!(f, "execute_throw"),
            &ControlInstruction::IsCall(r) =>
                write!(f, "is_call {}", r),
            &ControlInstruction::IsExecute(r) =>
                write!(f, "is_execute {}", r),
        }
    }
}

impl fmt::Display for IndexedChoiceInstruction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &IndexedChoiceInstruction::Try(offset) =>
                write!(f, "try {}", offset),
            &IndexedChoiceInstruction::Retry(offset) =>
                write!(f, "retry {}", offset),
            &IndexedChoiceInstruction::Trust(offset) =>
                write!(f, "trust {}", offset)
        }
    }
}


impl fmt::Display for BuiltInInstruction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &BuiltInInstruction::CleanUpBlock =>
                write!(f, "clean_up_block"),
            &BuiltInInstruction::DuplicateTerm =>
                write!(f, "duplicate_term X1"),
            &BuiltInInstruction::EraseBall =>
                write!(f, "erase_ball"),
            &BuiltInInstruction::Fail =>
                write!(f, "false"),
            &BuiltInInstruction::GetBall =>
                write!(f, "get_ball X1"),
            &BuiltInInstruction::GetCurrentBlock =>
                write!(f, "get_current_block X1"),
            &BuiltInInstruction::InstallNewBlock =>
                write!(f, "install_new_block"),
            &BuiltInInstruction::InternalCallN =>
                write!(f, "internal_call_N"),
            &BuiltInInstruction::IsAtomic(r) =>
                write!(f, "is_atomic {}", r),
            &BuiltInInstruction::IsVar(r) =>
                write!(f, "is_var {}", r),
            &BuiltInInstruction::ResetBlock =>
                write!(f, "reset_block"),
            &BuiltInInstruction::SetBall =>
                write!(f, "set_ball"),
            &BuiltInInstruction::Succeed =>
                write!(f, "true"),
            &BuiltInInstruction::UnwindStack =>
                write!(f, "unwind_stack"),
            &BuiltInInstruction::Unify =>
                write!(f, "unify"),
        }
    }
}

impl fmt::Display for ChoiceInstruction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &ChoiceInstruction::TryMeElse(offset) =>
                write!(f, "try_me_else {}", offset),
            &ChoiceInstruction::RetryMeElse(offset) =>
                write!(f, "retry_me_else {}", offset),
            &ChoiceInstruction::TrustMe =>
                write!(f, "trust_me")
        }
    }
}

impl fmt::Display for IndexingInstruction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &IndexingInstruction::SwitchOnTerm(v, c, l, s) =>
                write!(f, "switch_on_term {}, {}, {}, {}", v, c, l, s),
            &IndexingInstruction::SwitchOnConstant(num_cs, _) =>
                write!(f, "switch_on_constant {}", num_cs),
            &IndexingInstruction::SwitchOnStructure(num_ss, _) =>
                write!(f, "switch_on_structure {}", num_ss)
        }
    }
}

impl fmt::Display for ArithmeticTerm {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &ArithmeticTerm::Reg(r) => write!(f, "{}", r),
            &ArithmeticTerm::Interm(i) => write!(f, "@{}", i),
            &ArithmeticTerm::Float(fl) => write!(f, "{}", fl),
            &ArithmeticTerm::Integer(ref bi) => write!(f, "{}", bi)
        }
    }
}

impl fmt::Display for ArithmeticInstruction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &ArithmeticInstruction::Add(ref a1, ref a2, ref t) =>
                write!(f, "add {}, {}, @{}", a1, a2, t),
            &ArithmeticInstruction::Sub(ref a1, ref a2, ref t) =>
                write!(f, "sub {}, {}, @{}", a1, a2, t),
            &ArithmeticInstruction::Mul(ref a1, ref a2, ref t) =>
                write!(f, "mul {}, {}, @{}", a1, a2, t),
            &ArithmeticInstruction::Div(ref a1, ref a2, ref t) =>
                write!(f, "div {}, {}, @{}", a1, a2, t),
            &ArithmeticInstruction::IDiv(ref a1, ref a2, ref t) =>
                write!(f, "idiv {}, {}, @{}", a1, a2, t),
            &ArithmeticInstruction::RDiv(ref a1, ref a2, ref t) =>
                write!(f, "rdiv {}, {}, @{}", a1, a2, t),
            &ArithmeticInstruction::Neg(ref a, ref t) =>
                write!(f, "neg {}, @{}", a, t)
        }
    }
}

impl fmt::Display for CutInstruction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &CutInstruction::Cut(_) =>
                write!(f, "cut"),
            &CutInstruction::NeckCut(_) =>
                write!(f, "neck_cut"),
            &CutInstruction::GetLevel =>
                write!(f, "get_level")
        }
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Level::Shallow => write!(f, "A"),
            &Level::Deep => write!(f, "X")
        }
    }
}

impl fmt::Display for VarReg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &VarReg::Norm(RegType::Perm(reg)) => write!(f, "Y{}", reg),
            &VarReg::Norm(RegType::Temp(reg)) => write!(f, "X{}", reg),
            &VarReg::ArgAndNorm(RegType::Perm(reg), arg) =>
                write!(f, "Y{} A{}", reg, arg),
            &VarReg::ArgAndNorm(RegType::Temp(reg), arg) =>
                write!(f, "X{} A{}", reg, arg)
        }
    }
}

impl fmt::Display for RegType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &RegType::Perm(val) => write!(f, "Y{}", val),
            &RegType::Temp(val) => write!(f, "X{}", val)
        }
    }
}

#[allow(dead_code)]
pub fn print_code(code: &Code) {
    for clause in code {
        match clause {
            &Line::Arithmetic(ref arith) =>
                println!("{}", arith),
            &Line::Fact(ref fact) =>
                for fact_instr in fact {
                    println!("{}", fact_instr);
                },
            &Line::BuiltIn(ref instr) =>
                println!("{}", instr),
            &Line::Cut(ref cut) =>
                println!("{}", cut),
            &Line::Choice(ref choice) =>
                println!("{}", choice),
            &Line::Control(ref control) =>
                println!("{}", control),
            &Line::IndexedChoice(ref choice) =>
                println!("{}", choice),
            &Line::Indexing(ref indexing) =>
                println!("{}", indexing),
            &Line::Query(ref query) =>
                for query_instr in query {
                    println!("{}", query_instr);
                }
        }
    }
}

pub fn read() -> String {
    let _ = stdout().flush();

    let mut buffer = String::new();
    let mut result = String::new();

    let stdin = stdin();
    stdin.read_line(&mut buffer).unwrap();

    if &*buffer.trim() == ":{" {
        buffer.clear();

        stdin.read_line(&mut buffer).unwrap();

        while &*buffer.trim() != "}:" {
            result += buffer.as_str();
            buffer.clear();
            stdin.read_line(&mut buffer).unwrap();
        }
    } else {
        result = buffer;
    }

    result
}

pub fn eval<'a, 'b: 'a>(wam: &'a mut Machine, tl: &'b TopLevel) -> EvalSession<'b>
{
    match tl {
        &TopLevel::Declaration(ref decl) =>
            wam.submit_decl(decl),
        &TopLevel::Predicate(ref clauses) => {
            let mut cg = CodeGenerator::<DebrayAllocator>::new();

            let compiled_pred = match cg.compile_predicate(clauses) {
                Ok(pred) => pred,
                Err(e)   => return EvalSession::ParserError(e)
            };

            wam.add_predicate(clauses, compiled_pred)
        },
        &TopLevel::Fact(ref fact) => {
            let mut cg = CodeGenerator::<DebrayAllocator>::new();

            let compiled_fact = cg.compile_fact(fact);
            wam.add_fact(fact, compiled_fact)
        },
        &TopLevel::Rule(ref rule) => {
            let mut cg = CodeGenerator::<DebrayAllocator>::new();

            let compiled_rule = match cg.compile_rule(rule) {
                Ok(rule) => rule,
                Err(e) => return EvalSession::ParserError(e)                
            };

            wam.add_rule(rule, compiled_rule)
        },
        &TopLevel::Query(ref query) => {
            let mut cg = CodeGenerator::<DebrayAllocator>::new();

            let compiled_query = match cg.compile_query(query) {
                Ok(query) => query,
                Err(e) => return EvalSession::ParserError(e)
            };

            wam.submit_query(compiled_query, cg.take_vars())
        }
    }
}

fn error_string(e: &String) -> String {
    format!("error: exception thrown: {}", e)
}

pub fn print(wam: &mut Machine, result: EvalSession) {
    match result {
        EvalSession::InitialQuerySuccess(alloc_locs, mut heap_locs) => {
            print!("true");

            if !wam.or_stack_is_empty() {
                print!(" ");
            }

            println!(".");

            if heap_locs.is_empty() {
                return;
            }

            loop {
                let mut result = EvalSession::QueryFailure;
                let bindings = wam.heap_view(&heap_locs);

                let stdin  = stdin();
                let mut stdout = stdout().into_raw_mode().unwrap();

                write!(stdout, "{}", bindings).unwrap();
                stdout.flush().unwrap();

                if !wam.or_stack_is_empty() {
                    stdout.flush().unwrap();

                    for c in stdin.keys() {
                        match c.unwrap() {
                            Key::Char(' ') => {
                                write!(stdout, " ;\n\r").unwrap();
                                result = wam.continue_query(&alloc_locs, &mut heap_locs);
                                break;
                            },
                            Key::Char('.') => {
                                write!(stdout, " .\n\r").unwrap();
                                return;
                            },
                            _ => {}
                        }
                    }

                    if let &EvalSession::QueryFailure = &result {
                        write!(stdout, "false.\n\r").unwrap();
                        stdout.flush().unwrap();
                        return;
                    }

                    if let &EvalSession::QueryFailureWithException(ref e) = &result {
                        write!(stdout, "{}\n\r", error_string(e)).unwrap();
                        stdout.flush().unwrap();
                        return;
                    }
                } else {
                    break;
                }
            }

            write!(stdout(), ".\n").unwrap();
        },
        EvalSession::QueryFailure => println!("false."),
        EvalSession::QueryFailureWithException(e) => println!("{}", error_string(&e)),
        EvalSession::ImpermissibleEntry(msg) => println!("cannot overwrite builtin {}", msg),
        EvalSession::OpIsInfixAndPostFix => println!("cannot define an op to be both postfix and infix."),
        EvalSession::NamelessEntry => println!("the predicate head is not an atom or clause."),
        EvalSession::ParserError(e) => println!("{:?}", e),
        _ => {}
    };
}

// For linux 64 nasm

use crate::{
    parser::Type, strikeerror::{ASM_SYNTAX, COMP_ERR, COMP_INST, NOT_IMPLEMENTED},
};

use super::*;

#[derive(PartialEq, Clone)]
enum KnownVal {
    Int(i64),
    Str {
        raw: String, // Track the actual value
        len: usize,
    },
} // Values known at compile time

#[derive(PartialEq, Clone)]
enum DynamicVal {
    Int { data_addr: String },

    Str { data_addr: String, len_addr: String },
} // Values not known at compile time (e.g input() -> unknown string) OR values requested to be mutable

#[derive(PartialEq, Clone)]
enum Value {
    Known { val: KnownVal },
    Dynamic { val: DynamicVal },
}

#[derive(PartialEq, Clone)]
struct Variable {
    id: String,
    val: Value,
    t: Type,
}

struct CompilerScope {
    vars: Vec<Variable>,
    funcs: Vec<AST>,
    procs: Vec<AST>,
}

impl CompilerScope {
    pub fn new() -> CompilerScope {
        CompilerScope {
            vars: Vec::new(),
            funcs: Vec::new(),
            procs: Vec::new(),
        }
    }
}
pub struct Compiler {
    filename: String,
    rawcode: String,
    ast: Vec<(AST, Span)>,
    scopes: Vec<CompilerScope>, // Push recent scopes, so global var changes affect actual scopes

    rodata: String,
    data: String,
    bss: String,
    text: String,

    reg_strs: Vec<(String, String)>, // Address, value
    reg_mut_strs: Vec<(String, usize)>, // Address, capacity

    rodata_str_idx: usize,
    data_str_idx: usize,
}

impl Compiler {
    pub fn new(fname: String, code: String, ast: Vec<(AST, Span)>) -> Compiler {
        Compiler {
            filename: fname,
            rawcode: code,
            ast,
            scopes: vec![CompilerScope::new()],
            rodata: String::new(),
            data: String::new(),
            bss: String::from(
                "\n\tibuf resq 1", // Buffer for evaluating int expressions
            ),
            text: String::new(),
            reg_strs: Vec::new(),
            reg_mut_strs: Vec::new(),
            rodata_str_idx: 0,
            data_str_idx: 0,
        }
    }

    fn error(&self, code: u16, loc: Span, msg: &str) -> Error {
        Error::from_code(
            self.filename.clone(),
            self.rawcode.clone(),
            &code,
            Some(loc),
            msg,
        )
    }

    fn view_scope(&self) -> CompilerScope {
        let mut ret = CompilerScope::new();

        for scope in self.scopes.iter().rev() {
            for var in &scope.vars {
                if !ret.vars.contains(&var) {
                    ret.vars.push(var.clone());
                }
            }
            for func in &scope.funcs {
                if !ret.funcs.contains(&func) {
                    ret.funcs.push(func.clone());
                }
            }
            for proc in &scope.procs {
                if !ret.procs.contains(&proc) {
                    ret.procs.push(proc.clone());
                }
            }
        }

        ret
    }

    // Keep updating this!!!
    fn get_type(&self, node: &AST) -> Option<Type> {
        match node {
            AST::Value { val } => {
                Some(match val {
                    lexer::Value::Int(_) => Type::Int,
                    lexer::Value::Str(_) => Type::Str,
                    _ => Type::Int, // TEMP
                })
            }

            AST::VarGet { id } => {
                for var in self.view_scope().vars {
                    if var.id == *id {
                        return Some(var.t);
                    }
                }

                None
            }

            AST::BinOp { left, op, right } => {
                let ltype = self.get_type(left);
                let rtype = self.get_type(right);

                if ltype == rtype {
                    return ltype;
                }

                None
            }

            AST::UnaryOp { op, expr } => {
                if self.get_type(expr) == Some(Type::Int) {
                    return Some(Type::Int);
                }
                None
            }

            AST::ReqStr { expr } => Some(Type::Str),

            AST::ReqMutStr { expr } => Some(Type::Str),

            _ => None,
        }
    }

    fn is_numeric_expr(&self, node: &AST) -> bool {
        match node {
            AST::Value { val } => match val {
                lexer::Value::Int(_) => true,
                _ => false,
            },

            AST::VarGet { id } => {
                for var in self.view_scope().vars {
                    if var.id == *id {
                        return var.t == Type::Int;
                    }
                }

                false
            }

            AST::UnaryOp { op, expr } => self.is_numeric_expr(expr),
            AST::BinOp { left, op, right } => {
                self.is_numeric_expr(left) && self.is_numeric_expr(right)
            }

            _ => false,
        }
    }

    fn is_str_expr(&self, node: &AST) -> bool {
        match node {
            AST::Value { val } => match val {
                lexer::Value::Str(_) => true,
                _ => false,
            },

            AST::VarGet { id } => {
                for var in self.view_scope().vars {
                    if var.id == *id {
                        return var.t == Type::Str;
                    }
                }

                false
            }

            AST::BinOp { left, op, right } => self.is_str_expr(left) && self.is_str_expr(right),

            _ => false,
        }
    }

    fn eval_static_num(&self, node: &AST) -> i64 {
        // Later add float support
        match node {
            AST::Value { val } => {
                if let lexer::Value::Int(i) = val {
                    return *i as i64;
                }
            }

            AST::VarGet { id } => {
                if let Some(v) = self.get_var(id.clone()) {
                    match v.val {
                        Value::Known { val } => {
                            if let KnownVal::Int(i) = val {
                                return i;
                            }
                        }

                        Value::Dynamic { val } => {
                            return 0;
                        }
                    }
                }
            }

            AST::UnaryOp { op, expr } => {
                let val = self.eval_static_num(expr);

                match op {
                    Token::Sub => return -val,
                    _ => return val,
                }
            }

            AST::BinOp { left, op, right } => {
                let lval = self.eval_static_num(left);
                let rval = self.eval_static_num(right);

                match op {
                    Token::Add => return lval + rval,
                    Token::Sub => return lval - rval,
                    Token::Mul => return lval * rval,
                    Token::Div => return lval / rval, // Careful with div for now
                    _ => return 0,
                }
            }

            _ => (),
        }

        0
    }

    fn eval_static_str(&mut self, node: &AST, loc: Span) -> Result<String, Error> {
        match node {
            AST::Value { val } => {
                if let lexer::Value::Str(s) = val {
                    return Ok(String::from(s));
                }
            }

            AST::VarGet { id } => {
                if let Some(v) = self.get_var(id.clone()) {
                    match v.val {
                        Value::Known { val } => {
                            if let KnownVal::Str { raw, len } = val {
                                return Ok(raw);
                            }
                        }

                        Value::Dynamic { val } => {
                            return Err(self.error(COMP_ERR, loc, "Expected dynamic value"));
                        }
                    }
                }
            }

            AST::BinOp { left, op, right } => {
                let lval = self.eval_static_str(left, loc)?;
                let rval = self.eval_static_str(right, loc)?;

                match op {
                    Token::Add => return Ok(lval + &rval),
                    _ => return Err(self.error(COMP_ERR, loc, "+ is the only valid string-to-string operation"))
                }
            }

            AST::ReqStr { expr } => {
                let arg = self.eval_static_str(expr, loc)?;
                let addr = self.get_str(arg);

                return Ok(addr);
            }

            AST::ReqMutStr { expr } => {
                if self.is_static(expr) && self.is_numeric_expr(expr) {
                    let val = self.eval_static_num(&expr);
                    if val < 4 {
                        return Err(self.error(COMP_INST, loc, "Dynamic string allocations by reqmutstr have to be atleast 4 bytes"));
                    }
                    let addr = self.reqmutstr(val as usize);
                    return Ok(addr);
                }
                return Err(self.error(COMP_INST, loc, "Expected static integer"));
            
            }

            _ => (),
        }

        Err(self.error(COMP_ERR, loc, "Cannot resolve static string value"))
    }

    fn is_static(&self, node: &AST) -> bool {
        match node {
            AST::Value { val } => true, // If its just a raw value it must be static
            AST::VarGet { id } => {
                for var in self.view_scope().vars {
                    if var.id == *id {
                        match var.val {
                            Value::Known { .. } => return true,
                            Value::Dynamic { .. } => return false,
                        }
                    }
                }

                false
            }
            AST::UnaryOp { op, expr } => self.is_static(expr),
            AST::BinOp { left, op, right } => self.is_static(left) && self.is_static(right),
            AST::ReqStr { expr } | AST::ReqMutStr { expr } => true, // Req(Mut)Str returns same address, disregarding the value supplied
            _ => false,
        }
    }

    fn set_var(&mut self, var: Variable) {
        for mut scope in self.scopes.iter_mut().rev() {
            for v in scope.vars.iter_mut() {
                if v.id == var.id {
                    *v = var;
                    return;
                }
            }
        }

        let idx = self.scopes.len() - 1;
        self.scopes[idx].vars.push(var);
    }

    fn get_var(&self, id: String) -> Option<Variable> {
        for var in &self.view_scope().vars {
            if var.id == id {
                return Some(var.clone());
            }
        }

        None
    }

    fn reqmutstr(&mut self, bufsize: usize) -> String {
        let address = format!("_ms{}", self.data_str_idx());

        self.bss += &format!("\n\t{} resb {}", address, bufsize);
        self.bss += &format!("\n\t{}_len resq 1", address);

        self.reg_mut_strs.push((address.clone(), bufsize));
        address
    }

    fn compile_set_var(&mut self, node: AST, loc: Span) -> Result<(), Error> {
        if let AST::VarSet { id, expr } = &node {
            match self.get_type(expr) {
                Some(t) => {
                    match t {
                        Type::Int => {
                            if self.is_static(&expr) {
                                let val = self.eval_static_num(&expr);

                                self.set_var(Variable {
                                    id: id.clone(),
                                    val: Value::Known {
                                        val: KnownVal::Int(val),
                                    },
                                    t: Type::Int,
                                });
                                return Ok(());
                            }
                        }

                        Type::Str => {
                            if self.is_static(&expr) {
                                let val = self.eval_static_str(expr, loc)?;

                                self.set_var(Variable {
                                    id: id.clone(),
                                    val: Value::Known {
                                        val: KnownVal::Str {
                                            raw: val.clone(),
                                            len: val.len(),
                                        },
                                    },
                                    t: Type::Str,
                                });
                                return Ok(());
                            }
                        }

                        _ => return Err(Error::placeholder()),
                    };
                }

                None => {}
            }
        };

        Err(self.error(
            COMP_ERR,
            loc,
            "Unable to compile variable assignment statement",
        ))
    }

    fn data_str_idx(&mut self) -> usize {
        let ret = *&self.data_str_idx;
        self.data_str_idx += 1;
        ret
    }

    fn rodata_str_idx(&mut self) -> usize {
        let ret = *&self.rodata_str_idx;
        self.rodata_str_idx += 1;
        ret
    }

    // Looks if string value exists otherwise generates it
    fn get_str(&mut self, str: String) -> String {
        for (a, v) in &self.reg_strs {
            if str == *v {
                return a.clone();
            }
        }

        let address = format!("_rs{}", self.rodata_str_idx());
        self.rodata += &format!("\n\t{} db \"{}\"", address, str);
        self.rodata += &format!("\n\t{}_len equ $ - {}", address, address);

        self.reg_strs.push((address.clone(), str));

        return address;
    }

    fn compile_exit(&mut self, node: AST, loc: Span) -> Result<(), Error> {
        // Don't do type checking again, parser already does it.
        // Actually maybe do cuz idk what eval can return
        if let AST::Exit { code } = node {
            if self.is_numeric_expr(&*code) && self.is_static(&*code) {
                let val = self.eval_static_num(&*code);

                self.text += "\n\tmov rax, 60";
                self.text += &format!("\n\tmov rdi, {}", val.to_string());
                self.text += "\n\tsyscall";
                return Ok(());
            }
        }
        Err(self.error(COMP_ERR, loc, "Unable to compile exit statement"))
    }

    fn compile_asmblock(&mut self, node: AST, loc: Span) -> Result<(), Error> {
        if let AST::ASM { body } = node {
            let mut parsed = String::new();
            let mut idx = 0 as usize;
            let mut chars: Vec<char> = body.chars().collect();

            while idx < chars.len() {
                if chars[idx] == '{' {
                    idx += 1;
                    let mut closed = false;
                    let mut buf = String::new();
                    while idx < chars.len() {
                        if chars[idx] == '}' {
                            idx += 1;
                            closed = true;
                            break;
                        }
                        buf += &chars[idx].to_string();
                        idx += 1;
                    }

                    if !closed {
                        return Err(self.error(
                            ASM_SYNTAX,
                            loc,
                            "Assembly identifier injection block not closed ({ ID })",
                        ));
                    }

                    if let Some(Variable { id, val, t }) = self.get_var(String::from(buf.trim())) {
                        if t == Type::Str {
                            if let Value::Known { val } = val {
                                if let KnownVal::Str { raw, len } = val {
                                    parsed += &raw;
                                    continue;
                                }
                            }
                            return Err(self.error(ASM_SYNTAX, loc, "Assembly injection blocks only accept STATIC string variables (known values at compile time)"));
                        }
                        return Err(self.error(
                            ASM_SYNTAX,
                            loc,
                            "Assembly injection blocks only accept string variables",
                        ));
                    } else {
                        return Err(self.error(
                            ASM_SYNTAX,
                            loc,
                            &format!("Cannot find {} in current scope", buf.trim()),
                        ));
                    }
                }

                parsed += &chars[idx].to_string();
                idx += 1;
            }

            self.text += "\n\t; asm block (@{ ... })";
            self.text += &parsed;
            return Ok(());
        }

        Err(self.error(COMP_ERR, loc, "Unable to compile inline assembly block"))
    }

    fn compile_stmt(&mut self, stmt: AST, span: Span) -> Result<(), Error> {
        match stmt {
            AST::Exit { .. } => match self.compile_exit(stmt, span) {
                Ok(_) => Ok(()),
                Err(e) => return Err(e),
            },

            AST::VarSet { .. } => match self.compile_set_var(stmt, span) {
                Ok(_) => Ok(()),
                Err(e) => return Err(e),
            },

            AST::ASM { .. } => match self.compile_asmblock(stmt, span) {
                Ok(_) => Ok(()),
                Err(e) => return Err(e),
            },

            _ => {
                return Err(self.error(NOT_IMPLEMENTED, span, "Can't compile this statement"));
            }
        }
    }

    pub fn compile(&mut self) -> Result<String, Error> {
        // Clone takes performance but its a compiler so who cares
        for (stmt, span) in self.ast.clone() {
            if let Err(e) = self.compile_stmt(stmt, span) {
                return Err(e);
            }
        }

        self.compile_stmt(
            AST::Exit {
                code: Box::new(AST::Value {
                    val: lexer::Value::Int(0),
                }),
            },
            Span::zero(),
        );

        let mut ret = String::new();

        ret += ";compiled with strikelang ;)";
        ret += "\nsection .rodata";
        // Special characters
            ret += "\n\tnewln db 10";
            ret += "\n\tnewln_len equ $ - newln";
        ret += &self.rodata;
        ret += "\nsection .data";
        ret += &self.data;
        ret += "\nsection .bss";
        ret += &self.bss;
        ret += "\nsection .text\n\tglobal _start\n\t_start:";
        ret += &self.text;

        Ok(ret)
    }
}

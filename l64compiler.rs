// For linux 64 nasm
use crate::{
    parser::Type, strikeerror::{ASM_SYNTAX, COMP_ERR, COMP_INST, NOT_IMPLEMENTED, OVERFLOW_ERR, TYPE_ERR},
};

use super::*;

#[derive(PartialEq, Clone)]
enum KnownVal {
    Int(i64),
    Str {
        raw: String, // Track the actual value
        len: usize,
    },
    Bool(bool),
} // Values known at compile time

#[derive(PartialEq, Clone)]
enum DynamicVal {
    Int { data_addr: String },

    Str { data_addr: String, len_addr: String, len: usize }, // len is to statically track for bufs

    Bool { data_addr: String }, // a byte thats 0 or 1
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

    reg_strs: Vec<(String, String)>,    // Address, value
    reg_mut_str_bufs: Vec<(String, usize, bool)>, // Buffer address/name, capacity, in use/will be used again

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
            bss: String::new(),
            text: String::new(),
            reg_strs: Vec::new(),
            reg_mut_str_bufs: Vec::new(),
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
            AST::Value { val } => Some(match val {
                lexer::Value::Int(_) => Type::Int,
                lexer::Value::Str(_) => Type::Str,
                lexer::Value::Bool(_) => Type::Bool,
                _ => return None,
            }),

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

                match op {
                    Token::CompareEq
                    | Token::NotEq
                    | Token::Not
                    | Token::Greater
                    | Token::Less
                    | Token::GreatEq
                    | Token::LessEq
                    | Token::And
                    | Token::Or => {
                        return Some(Type::Bool);
                    }

                    _ => {}
                };

                if ltype == rtype {
                    return ltype;
                }

                None
            }

            AST::UnaryOp { op, expr } => {
                if self.get_type(expr) == Some(Type::Int) {
                    return Some(Type::Int);
                }

                if self.get_type(expr) == Some(Type::Bool) {
                    return Some(Type::Bool);
                }

                None // Strings shouldn't have unary operators
            }

            AST::ReqStr { .. } | AST::ReqMutStr { .. } | AST::DynaStr { .. } | AST::DynaAddr { .. }  => Some(Type::Str),

            AST::IsStatic { .. } => Some(Type::Bool),

            _ => None,
        }
    }

    fn eval_static_num(&self, node: &AST, loc: Span) -> Result<i64, Error> {
        // Later add float support
        match node {
            AST::Value { val } => {
                if let lexer::Value::Int(i) = val {
                    return Ok(*i as i64);
                }
            }

            AST::VarGet { id } => {
                if let Some(v) = self.get_var(id.clone()) {
                    match v.val {
                        Value::Known { val } => {
                            if let KnownVal::Int(i) = val {
                                return Ok(i);
                            }
                        }

                        Value::Dynamic { val } => {} // Falls back to error
                    }
                }
            }

            AST::UnaryOp { op, expr } => {
                let val = self.eval_static_num(expr, loc)?;

                match op {
                    Token::Sub => return Ok(-val),
                    _ => {
                        return Err(self.error(
                            COMP_ERR,
                            loc,
                            "Invalid unary operation on static number",
                        ))
                    }
                };
            }

            AST::BinOp { left, op, right } => {
                let lval = self.eval_static_num(left, loc)?;
                let rval = self.eval_static_num(right, loc)?;

                return Ok(match op {
                    Token::Add => lval + rval,
                    Token::Sub => lval - rval,
                    Token::Mul => lval * rval,
                    Token::Div => lval / rval, // Careful with div for now
                    _ => {
                        return Err(self.error(
                            COMP_ERR,
                            loc,
                            "Unexpected numerical number operator",
                        ))
                    }
                });
            }

            _ => {}
        };
        return Err(self.error(COMP_ERR, loc, "Cannot resolve static integer value"));
    }

    fn eval_static_str(&mut self, node: &AST, loc: Span, nomut: bool) -> Result<String, Error> {
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
                            return Err(self.error(COMP_ERR, loc, "Expected static value"));
                        }
                    }
                }
            }

            AST::BinOp { left, op, right } => {
                let lval = self.eval_static_str(left, loc, nomut)?;
                let rval = self.eval_static_str(right, loc, nomut)?;

                match op {
                    Token::Add => return Ok(lval + &rval),
                    _ => {
                        return Err(self.error(
                            COMP_ERR,
                            loc,
                            "+ is the only valid string-to-string operation",
                        ))
                    }
                }
            }

            AST::ReqStr { expr } => {
                if nomut {
                    return Ok(String::from("_reg_"));
                }
                let arg = self.eval_static_str(expr, loc, nomut)?;
                let addr = self.get_str(arg);

                return Ok(addr);
            }

            AST::ReqMutStr { expr } => {
                if nomut {
                    return Ok(String::from("_reg_"));
                }
                if self.is_static(expr) && self.get_type(expr) == Some(Type::Int) {
                    let val = self.eval_static_num(&expr, loc)?;
                    if val < 4 {
                        return Err(self.error(
                            COMP_INST,
                            loc,
                            "Dynamic string allocations by reqmutstr have to be atleast 4 bytes",
                        ));
                    }

                    let addr = self.reqmutstr(val as usize);
                    return Ok(addr);
                }
                return Err(self.error(COMP_INST, loc, "Expected static integer"));
            }

            AST::DynaAddr { expr } => {
                if self.is_static(expr) {
                    return Err(self.error(TYPE_ERR, loc, "dynaaddr expects a dynamic value; use isstatic{ANY} to check if a value is dynamic"));
                }

                if let Some(t) = self.get_type(expr) {
                    match t {
                        Type::Bool => {
                            // TODO: eval_dynamic bool
                        }

                        Type::Int => {
                            // TODO: eval dynamic int
                        }

                        Type::Str => {
                            return Ok(self.eval_dynamic_str(expr, loc)?.0);
                        }

                        _ => {}
                    }
                }
            }

            _ => (),
        };

        Err(self.error(COMP_ERR, loc, "Cannot resolve static string value"))
    }

    
    fn eval_static_bool(&mut self, node: &AST, loc: Span) -> Result<bool, Error> {
        match node {
            AST::Value { val } => {
                if let lexer::Value::Bool(b) = val {
                    return Ok(*b);
                }
            }

            AST::VarGet { id } => {
                if let Some(v) = self.get_var(id.clone()) {
                    match v.val {
                        Value::Known { val } => {
                            if let KnownVal::Bool(b) = val {
                                return Ok(b);
                            }
                        }

                        Value::Dynamic { val } => {
                            return Err(self.error(COMP_ERR, loc, "Expected static value"));
                        }
                    }
                }
            }

            AST::BinOp { left, op, right } => {
                if *op == Token::NotEq {
                    // TODO Add Int-Float conversion if self.get_type(left) == (Some(Type::Int) | Some(Type::Float))

                    if self.get_type(left) == self.get_type(right) && self.get_type(left) != None {
                        match self.get_type(left).unwrap() {
                            Type::Bool => {
                                let lval = self.eval_static_bool(left, loc)?;
                                let rval = self.eval_static_bool(right, loc)?;

                                return Ok(lval != rval);
                            }

                            Type::Int => {
                                let lval = self.eval_static_num(left, loc)?;
                                let rval = self.eval_static_num(right, loc)?;

                                return Ok(lval != rval);
                            }

                            Type::Str => {
                                let lval = self.eval_static_str(left, loc, false)?;
                                let rval = self.eval_static_str(right, loc, false)?;

                                return Ok(lval != rval);
                            }

                            _ => {}
                        };
                    }
                    return Err(self.error(
                        TYPE_ERR,
                        loc,
                        "!= is not supported for the given operand types",
                    ));
                }

                if *op == Token::CompareEq {
                    // TODO Add Int-Float conversion if self.get_type(left) == (Some(Type::Int) | Some(Type::Float))

                    if self.get_type(left) == self.get_type(right) && self.get_type(left) != None {
                        match self.get_type(left).unwrap() {
                            Type::Bool => {
                                let lval = self.eval_static_bool(left, loc)?;
                                let rval = self.eval_static_bool(right, loc)?;

                                return Ok(lval == rval);
                            }

                            Type::Int => {
                                let lval = self.eval_static_num(left, loc)?;
                                let rval = self.eval_static_num(right, loc)?;

                                return Ok(lval == rval);
                            }

                            Type::Str => {
                                let lval = self.eval_static_str(left, loc, false)?;
                                let rval = self.eval_static_str(right, loc, false)?;

                                return Ok(lval == rval);
                            }

                            _ => {}
                        };
                    }

                    return Err(self.error(
                        TYPE_ERR,
                        loc,
                        "== is not supported for the given operand types",
                    ));
                }

                if self.get_type(left) == Some(Type::Bool)
                    && self.get_type(right) == Some(Type::Bool)
                {
                    let lval = self.eval_static_bool(left, loc)?;
                    let rval = self.eval_static_bool(right, loc)?;

                    return Ok(match op {
                        Token::And => lval && rval,

                        Token::Or => lval || rval,

                        _ => {
                            return Err(self.error(
                                TYPE_ERR,
                                loc,
                                "&&, &, ||, and | are the only valid bool-to-bool operators",
                            ));
                        }
                    });
                }

                if self.get_type(left) == Some(Type::Int) && self.get_type(right) == Some(Type::Int)
                {
                    let lval = self.eval_static_num(left, loc)?;
                    let rval = self.eval_static_num(right, loc)?;
                    return Ok(match op {
                        Token::GreatEq => {
                            lval >= rval
                        }
    
                        Token::Greater => {
                            lval > rval
                        }
    
                        Token::LessEq => {
                            lval <= rval
                        }
    
                        Token::Less => {
                            lval < rval
                        }
    
                        _ => {
                            return Err(self.error(TYPE_ERR, loc, ">=, >, <=, and < are the only valid boolean operators for numerical values"))
                        }
                    });
                }
            }

            AST::UnaryOp { op, expr } => {
                if *op != Token::Not {
                    return Err(self.error(
                        TYPE_ERR,
                        loc,
                        "! is the only valid unary operator for booleans",
                    ));
                }

                if self.get_type(&expr) != Some(Type::Bool) {
                    return Err(self.error(TYPE_ERR, loc, "! only works on booleans"));
                }

                return Ok(!self.eval_static_bool(expr, loc)?);
            }

            AST::IsStatic { expr } => {
                if self.get_type(expr) == None {
                    return Err(self.error(TYPE_ERR, loc, "isstatic{} needs a valid value"));
                }
                return Ok(self.is_static(expr));
            }

            _ => (),
        };

        Err(self.error(COMP_ERR, loc, "Cannot resolve static boolean value"))
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
            AST::ReqStr { .. } | AST::ReqMutStr { .. } | AST::IsStatic { .. } | AST::DynaAddr { .. } => true, // Req(Mut)Str returns same address, disregarding the value supplied

            AST::If {
                cond,
                body,
                branches,
                elsebranch,
            } => {
                if !self.is_static(cond) {
                    return false;
                }

                for stmt in body {
                    if !self.is_static(&*stmt) {
                        return false;
                    }
                }

                for branch in branches {
                    if !self.is_static(&*branch.0) {
                        return false;
                    }

                    for stmt in &branch.1 {
                        if !self.is_static(&*stmt) {
                            return false;
                        }
                    }
                }

                if let Some(e) = elsebranch {
                    for stmt in e {
                        if !self.is_static(&*stmt) {
                            return false;
                        }
                    }
                }

                return true;
            }
            _ => false,
        }
    }

    // leave as mut since it uses eval_static_str
    fn uses_addr(&mut self, node: &AST, addr: String, loc: Span) -> Result<bool, Error> {
        match node {
            AST::Exit { code } => {
                return Ok(self.uses_addr(code, addr, loc)?);
            }

            AST::VarGet { id } => {
                if let Some(var) = self.get_var(id.clone()) {
                    if let Value::Dynamic { val } = var.val {
                        match val {
                            DynamicVal::Int { data_addr } => {
                                return Ok(data_addr == addr);
                            }

                            DynamicVal::Str { data_addr, len_addr, len } => {
                                return Ok(data_addr == addr || len_addr == addr) ;
                            }

                            DynamicVal::Bool { data_addr } => {
                                return Ok(data_addr == addr);
                            }
                        };
                    }
                }
            }

            AST::BinOp { left, op, right } => {
                return Ok(self.uses_addr(left, addr.clone(), loc)? || self.uses_addr(right, addr, loc)?);
            }

            AST::UnaryOp { op, expr } => {
                return self.uses_addr(expr, addr.clone(), loc);
            }

            AST::If { cond, body, branches, elsebranch } => {
                if self.uses_addr(cond, addr.clone(), loc)? {
                    return Ok(true);
                }

                for stmt in body {
                    if self.uses_addr(stmt, addr.clone(), loc)? {
                        return Ok(true);
                    }
                }

                for branch in branches {
                    if self.uses_addr(&*branch.0, addr.clone(), loc)? {
                        return Ok(true);
                    }

                    for stmt in &branch.1 {
                        if self.uses_addr(&*stmt, addr.clone(), loc)? {
                            return Ok(true);
                        }
                    }
                }

                if let Some(e) = elsebranch {
                    for stmt in e {
                        if self.uses_addr(&*stmt, addr.clone(), loc)? {
                            return Ok(true);
                        }
                    }
                }

                return Ok(false);
            }

            AST::VarSet { id, expr } => {
                return self.uses_addr(expr, addr, loc);
            }

            AST::DynaStr { str_addr, len} => {
                if self.is_static(str_addr) {
                    let val = self.eval_static_str(node, loc, true)?;

                    if val == addr {
                        return Ok(true);
                    }
                }
            }

            AST::DynaAddr { expr } => {
                // If a buffer is used in an expression, then it shouldn't be considered used again cuz the buffer is gonna be reused
                return Ok(self.uses_addr(expr, addr, loc)?);
            }

            _ => { }
        }

        Ok(false)
    }

    fn addr_used_again(&mut self, addr: String, stmt_idx: usize, loc: Span) -> Result<bool, Error> {
        for i in stmt_idx + 1..self.ast.len() {
            if self.is_static(&self.ast[i].0) {
                continue;
            }
            if let Ok(true) = self.uses_addr(&self.ast[i].0.clone(), addr.clone(), loc) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn get_str_buf(&mut self, size: usize) -> String {
        // Search through string buffers, if one is not in use then make its size atleast that needed and return it
        for buf in self.reg_mut_str_bufs.iter_mut() {
            if !buf.2 {
                if buf.1 < size {
                    buf.1 = size;
                }
                buf.2 = true; // in use now
                return buf.0.clone();
            }
        }

        let addr = format!("_sb{}", self.data_str_idx());
        self.reg_mut_str_bufs.push((addr.clone(), size, true));

        addr
    }

    fn eval_dynamic_str(&mut self, node: &AST, loc: Span) -> Result<(String, usize, String), Error> { // Returns address of result, length IN CHARS (*4 bytes) OR address of length
        if self.is_static(node) { // Cuz dynamic str eval is recursive and values CAN be static
            let res = self.eval_static_str(node, loc, false)?;
            return Ok((self.get_str(res.clone()), res.len(), String::new()));
        }

        match node {
            AST::DynaStr { str_addr, len} => {
                // It is the developer's responsibility to make sure the addressses are valid
                if self.is_static(str_addr) {
                    if self.is_static(len) {
                        let val = self.eval_static_num(len, loc)?;
                        // TODO: Check if val is float
                        if val < 0 {
                            return Err(self.error(TYPE_ERR, loc, "dynastr requires positive integer length"));
                        }
                        return Ok((self.eval_static_str(str_addr, loc, false)?, val as usize, (self.eval_static_str(str_addr, loc, false)? + "_len")));
                    }
                    return Err(self.error(TYPE_ERR, loc, "dynastr requires a STATIC integer length;"));
                }
                return Err(self.error(TYPE_ERR, loc, "dynastr requires a STATIC string value;"));
            }

            AST::VarGet { id } => {
                if let Some(var) = self.get_var(id.clone()) {
                    match var.val {
                        Value::Dynamic { val } => {
                            if let DynamicVal::Str { data_addr, len_addr, len } = val {
                                return Ok((data_addr, len, len_addr));
                            }
                            return Err(self.error(TYPE_ERR, loc, &format!("'{}' is not a string", id)));
                        }

                        Value::Known { val } => {
                            if let KnownVal::Str { raw, len } = val {
                                return Ok((self.get_str(raw), len, String::new()));
                            }
                            return Err(self.error(TYPE_ERR, loc, &format!("'{}' is not a string", id)));
                        }
                    }
                }
            }

            AST::BinOp { left, op, right } => {
                let lres = self.eval_dynamic_str(left, loc)?;
                let rres = self.eval_dynamic_str(right, loc)?;

                match *op {
                    Token::Add => {
                        let buf = self.get_str_buf(lres.1 + rres.1);
                        
                        self.text += "\n\tcld";
                        self.text += &format!("\n\tmov rsi, {}", lres.0);
                        self.text += &format!("\n\tmov rdi, {}", buf);
                        if lres.2 == String::new() { // Length known at compile time/static
                            self.text += &format!("\n\tmov rcx, {}", lres.1);
                        } else {
                            self.text += &format!("\n\tmov ecx, dword [{}]", lres.2);
                        }
                        self.text += "\n\trep movsb";

                        self.text += &format!("\n\tmov rsi, {}", rres.0);
                        if rres.2 == String::new() { // Length known at compile time/static
                            self.text += &format!("\n\tmov rcx, {}", rres.1);
                        } else {
                            self.text += &format!("\n\tmov ecx, dword [{}]", rres.2);
                        }
                        self.text += "\n\trep movsb";
                        let mut both_static = false;
                        if lres.2 == String::new() {
                            if rres.2 == String::new() {
                                // Both static lengths
                                self.text += &format!("\n\tmov dword [{}_len], {}", buf, lres.1 + rres.1);
                                both_static = true;
                            } else {
                                // Static, dynamic
                                
                                self.text += &format!("\n\tmov eax, dword [{}]", rres.2);
                                self.text += &format!("\n\tadd eax, {}", lres.1);
                            }
                        } else {
                            if rres.2 == String::new() {
                                // Dynamic, static
                                self.text += &format!("\n\tmov eax, dword [{}]", lres.2);
                                self.text += &format!("\n\tadd eax, {}", rres.1);
                            } else {
                                // Both dynamic lengths
                                self.text += &format!("\n\tmov eax, dword [{}]", rres.2);
                                self.text += &format!("\n\tadd eax, dword [{}]", lres.2);
                            }
                        }

                        if !both_static {
                            self.text += &format!("\n\tmov dword [{}_len], eax", buf);
                        }
                        self.text += "\n\t";

                        return Ok((buf.clone(), lres.1 + rres.1 as usize, format!("{}_len", buf)));
                    }

                    _ => {
                        return Err(self.error(TYPE_ERR, loc, "+ (concatenate) is the only valid string-to-string operation"));
                    }
                };
            }

            _ => { }
        };
        
        Err(self.error(COMP_ERR, loc, "Cannot resolve dynamic string value"))
    }

    fn update_str_bufs(&mut self, stmt_idx: usize, loc: Span) -> Result<(), Error> {
        let mut bufs = self.reg_mut_str_bufs.clone();

        for buf in bufs.iter_mut() {
            if buf.2 {
                buf.2 = self.addr_used_again(buf.0.clone(), stmt_idx, loc)?;
            }
        }
        self.reg_mut_str_bufs = bufs;

        Ok(())
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
        self.bss += &format!("\n\t{}_len resd 1", address);

        address
    }

    fn compile_set_var(&mut self, node: AST, loc: Span) -> Result<(), Error> {
        if let AST::VarSet { id, expr } = &node {
            match self.get_type(expr) {
                Some(t) => {
                    match t {
                        Type::Int => {
                            if self.is_static(&expr) {
                                let val = self.eval_static_num(&expr, loc)?;

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
                                let val = self.eval_static_str(expr, loc, false)?;

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
                            
                            let (addr, len, len_addr) = self.eval_dynamic_str(&expr, loc)?;
                            
                            self.set_var(Variable { id: id.clone(), val: Value::Dynamic { val: DynamicVal::Str { data_addr: addr, len_addr: len_addr, len: len } }, t });
                            return Ok(());
                        }

                        Type::Bool => {
                            if self.is_static(&expr) {
                                let val = self.eval_static_bool(expr, loc)?;

                                self.set_var(Variable {
                                    id: id.clone(),
                                    val: Value::Known {
                                        val: KnownVal::Bool(val),
                                    },
                                    t: Type::Bool,
                                });
                                return Ok(());
                            }
                        }

                        _ => return Err(Error::placeholder()),
                    };
                }

                None => {}
            };
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
        if let AST::Exit { code } = node {
            if self.get_type(&*code) == Some(Type::Int) && self.is_static(&*code) {
                let val = self.eval_static_num(&*code, loc)?;

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

    fn compile_if(&mut self, node: AST, loc: Span) -> Result<(), Error> {
        if let AST::If {
            cond,
            body,
            branches,
            elsebranch,
        } = node
        {
            if self.get_type(&cond) == Some(Type::Bool) {
                for b in &branches {
                    if self.get_type(&b.0) != Some(Type::Bool) {
                        return Err(self.error(
                            TYPE_ERR,
                            loc,
                            "Elif condition must evaluate to a valid boolean",
                        ));
                    }
                }

                if self.is_static(&cond) {
                    let val = self.eval_static_bool(&cond, loc)?;

                    if val {
                        self.scopes.push(CompilerScope::new());
                        for stmt in &body {
                            self.compile_stmt(*stmt.clone(), loc)?;
                        }
                        self.scopes.remove(self.scopes.len() - 1);
                        return Ok(());
                    }

                    for branch in branches {
                        if self.is_static(&branch.0) {
                            let val = self.eval_static_bool(&branch.0, loc)?;

                            if val {
                                self.scopes.push(CompilerScope::new());
                                for stmt in branch.1 {
                                    self.compile_stmt(*stmt.clone(), loc)?;
                                }
                                self.scopes.remove(self.scopes.len() - 1);
                                return Ok(());
                            }
                        } else {
                            // TODO: dynamic elifs
                        }
                    }

                    if let Some(stmts) = elsebranch {
                        self.scopes.push(CompilerScope::new());
                        for stmt in stmts {
                            self.compile_stmt(*stmt.clone(), loc)?;
                        }
                        self.scopes.remove(self.scopes.len() - 1);
                    }

                    return Ok(());
                }

                // TODO: make dynamic ifs
            } else {
                return Err(self.error(
                    TYPE_ERR,
                    loc,
                    "If condition must evaluate to a valid boolean",
                ));
            }
        }

        Err(self.error(COMP_ERR, loc, "Unable to compile if statement"))
    }

    fn compile_stmt(&mut self, stmt: AST, span: Span) -> Result<(), Error> {
        match stmt {
            AST::Exit { .. } => return self.compile_exit(stmt, span),

            AST::VarSet { .. } => return self.compile_set_var(stmt, span),

            AST::ASM { .. } => return self.compile_asmblock(stmt, span),

            AST::If { .. } => return self.compile_if(stmt, span),

            _ => {
                return Err(self.error(NOT_IMPLEMENTED, span, "Can't compile this statement"));
            }
        }
    }

    pub fn compile(&mut self) -> Result<String, Error> {
        // Clone takes performance but its a compiler so who cares
        for (idx, (stmt, span)) in self.ast.clone().iter().enumerate() {
            if let Err(e) = self.compile_stmt(stmt.clone(), *span) {
                return Err(e);
            }
            self.update_str_bufs(idx, *span)?;
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
        for buf in &self.reg_mut_str_bufs {
            self.bss += &format!("\n\t{} resb {}", buf.0, buf.1);
            self.bss += &format!("\n\t{}_len resd 1", buf.0);
        }
        ret += &self.bss;
        ret += "\nsection .text\n\tglobal _start\n\t_start:";
        ret += &self.text;

        Ok(ret)
    }
}

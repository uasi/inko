//! LL(1) recursive-descent parser for Inko source code.

use std::collections::HashSet;
use lexer::{Lexer, Token, TokenType};

/// Macro for parsing a binary operation such as `x < y`.
macro_rules! binary_op {
    ($rec: expr, $start: expr, $child: ident, $($token_type: ident),+) => ({
        let mut node = $rec.$child($start)?;

        loop {
            let token_type = if let Some(token) = $rec.lexer.peek() {
                token.token_type.clone()
            } else {
                break;
            };

            match token_type {
                $(
                    TokenType::$token_type => {
                        let operator = $rec.lexer.next().unwrap();
                        let start = $rec.lexer.next().unwrap();
                        let rhs = $rec.$child(start)?;

                        node = Node::$token_type {
                            left: Box::new(node),
                            right: Box::new(rhs),
                            line: operator.line,
                            column: operator.column
                        };
                    }
                )+
                _ => break
            }
        }

        Ok(node)
    })
}

/// Returns a parse error.
///
/// The error's message is generated similar to println!(). For example:
///
///     parse_error!("Expected ], got {:?}", token.token_type);
///
/// This would produce an error with the message along the lines of:
///
///     "Expected ], got TokenType::Comma"
macro_rules! parse_error {
    ($msg: expr $(, $format_arg: expr)*) => ({
        return Err(format!($msg $(, $format_arg)*));
    })
}

/// Pulls a token from the lexer or returns an error in case of all input being
/// consumed.
macro_rules! next_or_error {
    ($parser: expr) => ({
        if let Some(token) = $parser.lexer.next() {
            token
        } else {
            parse_error!("Unexpected end of input");
        }
    })
}

/// Pulls a token from the lexer and asserts that it's of a given type.
macro_rules! next_of_type {
    ($parser: expr, $expected: expr) => ({
        let token = next_or_error!($parser);

        if token.token_type != $expected {
            parse_error!("Unexpected token {:?}, expected a {:?}",
                         token.token_type, $expected);
        }

        token
    })
}

/// Parses a value into the given type.
macro_rules! parse_value {
    ($vtype: ty, $value: expr) => ({
        $value.parse::<$vtype>().or_else(|err| Err(err.to_string()))
    })
}

/// Parses a comma, or breaks if the current token is of the given type.
macro_rules! comma_or_break_on {
    ($parser: expr, $btoken: pat) => ({
        if let Some(token) = $parser.lexer.next() {
            match token.token_type {
                TokenType::Comma => {}
                $btoken => break,
                _ => {
                    parse_error!("Unexpected token {:?}", token.token_type);
                }
            }
        } else {
            parse_error!("Unexpected end of input");
        }
    })
}

macro_rules! send_or {
    ($parser: expr, $start: expr, $alternative: expr) => ({
        if $parser.lexer.next_type_is(&TokenType::ParenOpen) {
            let args = $parser.arguments_with_parenthesis()?;

            Ok(Node::Send {
                name: $start.value,
                receiver: None,
                arguments: args,
                line: $start.line,
                column: $start.column,
            })
        } else {
            // If an identifier is followed by another expression on the same
            // line we'll treat said expression as the start of an argument
            // list.
            if $parser.next_expression_is_argument($start.line) {
                let args = $parser.arguments_without_parenthesis()?;

                Ok(Node::Send {
                    name: $start.value,
                    receiver: None,
                    arguments: args,
                    line: $start.line,
                    column: $start.column,
                })
            } else {
                Ok($alternative)
            }
        }
    })
}

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    message_tokens: HashSet<TokenType>,
    value_start: HashSet<TokenType>,
}

#[derive(Debug)]
pub enum Node {
    Expressions { nodes: Vec<Node> },

    Send {
        name: String,
        receiver: Option<Box<Node>>,
        arguments: Vec<Node>,
        line: usize,
        column: usize,
    },

    String {
        value: String,
        line: usize,
        column: usize,
    },

    Integer {
        value: i64,
        line: usize,
        column: usize,
    },

    Float {
        value: f64,
        line: usize,
        column: usize,
    },

    Array {
        values: Vec<Node>,
        line: usize,
        column: usize,
    },

    Hash {
        pairs: Vec<(Node, Node)>,
        line: usize,
        column: usize,
    },

    SelfObject { line: usize, column: usize },

    Identifier {
        name: String,
        line: usize,
        column: usize,
    },

    Attribute {
        name: String,
        line: usize,
        column: usize,
    },

    Constant {
        receiver: Option<Box<Node>>,
        name: String,
        line: usize,
        column: usize,
    },

    Comment {
        value: String,
        line: usize,
        column: usize,
    },

    Type {
        constant: Box<Node>,
        arguments: Vec<Node>,
        return_type: Option<Box<Node>>,
        line: usize,
        column: usize,
    },

    UnionType {
        types: Vec<Node>,
        line: usize,
        column: usize,
    },

    Closure {
        arguments: Vec<Node>,
        return_type: Option<Box<Node>>,
        body: Box<Node>,
        line: usize,
        column: usize,
    },

    ArgumentDefine {
        name: String,
        value_type: Option<Box<Node>>,
        default: Option<Box<Node>>,
        line: usize,
        column: usize,
        rest: bool,
    },

    KeywordArgument {
        name: String,
        value: Box<Node>,
        line: usize,
        column: usize,
    },

    Method {
        receiver: Option<Box<Node>>,
        name: String,
        arguments: Vec<Node>,
        type_arguments: Vec<Node>,
        return_type: Option<Box<Node>>,
        throw_type: Option<Box<Node>>,
        body: Option<Box<Node>>,
        line: usize,
        column: usize,
    },

    Class {
        name: String,
        type_arguments: Vec<Node>,
        implements: Vec<Node>,
        body: Box<Node>,
        line: usize,
        column: usize,
    },

    Trait {
        name: String,
        type_arguments: Vec<Node>,
        body: Box<Node>,
        line: usize,
        column: usize,
    },

    Implement {
        name: Box<Node>,
        type_arguments: Vec<Node>,
        renames: Vec<(Node, Node)>,
        line: usize,
        column: usize,
    },

    Return {
        value: Option<Box<Node>>,
        line: usize,
        column: usize,
    },

    LetDefine {
        name: Box<Node>,
        value: Box<Node>,
        value_type: Option<Box<Node>>,
        line: usize,
        column: usize,
    },

    VarDefine {
        name: Box<Node>,
        value: Box<Node>,
        value_type: Option<Box<Node>>,
        line: usize,
        column: usize,
    },

    Import {
        steps: Vec<Node>,
        symbols: Vec<Node>,
        line: usize,
        column: usize,
    },

    ImportSymbol {
        symbol: Box<Node>,
        alias: Option<Box<Node>>,
    },

    TypeCast {
        value: Box<Node>,
        target_type: Box<Node>,
        line: usize,
        column: usize,
    },

    TypeDefine {
        name: Box<Node>,
        value: Box<Node>,
        line: usize,
        column: usize,
    },

    Try {
        body: Box<Node>,
        else_body: Option<Box<Node>>,
        else_argument: Option<Box<Node>>,
        line: usize,
        column: usize,
    },

    Throw {
        value: Box<Node>,
        line: usize,
        column: usize,
    },

    Or {
        left: Box<Node>,
        right: Box<Node>,
        line: usize,
        column: usize,
    },

    And {
        left: Box<Node>,
        right: Box<Node>,
        line: usize,
        column: usize,
    },

    Equal {
        left: Box<Node>,
        right: Box<Node>,
        line: usize,
        column: usize,
    },

    NotEqual {
        left: Box<Node>,
        right: Box<Node>,
        line: usize,
        column: usize,
    },

    Lower {
        left: Box<Node>,
        right: Box<Node>,
        line: usize,
        column: usize,
    },

    LowerEqual {
        left: Box<Node>,
        right: Box<Node>,
        line: usize,
        column: usize,
    },

    Greater {
        left: Box<Node>,
        right: Box<Node>,
        line: usize,
        column: usize,
    },

    GreaterEqual {
        left: Box<Node>,
        right: Box<Node>,
        line: usize,
        column: usize,
    },

    BitwiseOr {
        left: Box<Node>,
        right: Box<Node>,
        line: usize,
        column: usize,
    },

    BitwiseXor {
        left: Box<Node>,
        right: Box<Node>,
        line: usize,
        column: usize,
    },

    BitwiseAnd {
        left: Box<Node>,
        right: Box<Node>,
        line: usize,
        column: usize,
    },

    ShiftLeft {
        left: Box<Node>,
        right: Box<Node>,
        line: usize,
        column: usize,
    },

    ShiftRight {
        left: Box<Node>,
        right: Box<Node>,
        line: usize,
        column: usize,
    },

    Add {
        left: Box<Node>,
        right: Box<Node>,
        line: usize,
        column: usize,
    },

    Sub {
        left: Box<Node>,
        right: Box<Node>,
        line: usize,
        column: usize,
    },

    Div {
        left: Box<Node>,
        right: Box<Node>,
        line: usize,
        column: usize,
    },

    Mod {
        left: Box<Node>,
        right: Box<Node>,
        line: usize,
        column: usize,
    },

    Mul {
        left: Box<Node>,
        right: Box<Node>,
        line: usize,
        column: usize,
    },

    Pow {
        left: Box<Node>,
        right: Box<Node>,
        line: usize,
        column: usize,
    },

    InclusiveRange {
        left: Box<Node>,
        right: Box<Node>,
        line: usize,
        column: usize,
    },

    ExclusiveRange {
        left: Box<Node>,
        right: Box<Node>,
        line: usize,
        column: usize,
    },

    Reassign {
        variable: Box<Node>,
        value: Box<Node>,
        line: usize,
        column: usize,
    },
}

pub type ParseResult = Result<Node, String>;

impl<'a> Parser<'a> {
    pub fn new(input: &str) -> Self {
        Parser {
            lexer: Lexer::new(input.chars().collect()),
            message_tokens: hash_set![TokenType::Add,
                                      TokenType::And,
                                      TokenType::BitwiseAnd,
                                      TokenType::BitwiseOr,
                                      TokenType::BitwiseXor,
                                      TokenType::Constant,
                                      TokenType::Div,
                                      TokenType::Equal,
                                      TokenType::ExclusiveRange,
                                      TokenType::Greater,
                                      TokenType::GreaterEqual,
                                      TokenType::Identifier,
                                      TokenType::Impl,
                                      TokenType::Import,
                                      TokenType::InclusiveRange,
                                      TokenType::Let,
                                      TokenType::Lower,
                                      TokenType::LowerEqual,
                                      TokenType::Mod,
                                      TokenType::Mul,
                                      TokenType::NotEqual,
                                      TokenType::Class,
                                      TokenType::Or,
                                      TokenType::Pow,
                                      TokenType::Return,
                                      TokenType::SelfObject,
                                      TokenType::ShiftLeft,
                                      TokenType::ShiftRight,
                                      TokenType::Sub,
                                      TokenType::Trait,
                                      TokenType::Var,
                                      TokenType::BracketOpen,
                                      TokenType::Throw,
                                      TokenType::Else],
            value_start: hash_set![TokenType::String,
                                   TokenType::Integer,
                                   TokenType::Float,
                                   TokenType::Identifier,
                                   TokenType::Constant,
                                   TokenType::HashOpen,
                                   TokenType::Sub,
                                   TokenType::BracketOpen,
                                   TokenType::CurlyOpen,
                                   TokenType::Function,
                                   TokenType::Let,
                                   TokenType::Let,
                                   TokenType::Class,
                                   TokenType::Trait,
                                   TokenType::Return,
                                   TokenType::Impl,
                                   TokenType::Comment,
                                   TokenType::Colon,
                                   TokenType::Type,
                                   TokenType::Attribute,
                                   TokenType::SelfObject,
                                   TokenType::Try,
                                   TokenType::Throw],
        }
    }

    pub fn line(&self) -> usize {
        self.lexer.line
    }

    pub fn column(&self) -> usize {
        self.lexer.column
    }

    /// Parses the input and returns an AST.
    pub fn parse(&mut self) -> ParseResult {
        self.expressions()
    }

    pub fn expressions(&mut self) -> ParseResult {
        let mut children = Vec::new();

        while let Some(token) = self.lexer.next() {
            children.push(self.import_or_expression(token)?);
        }

        Ok(Node::Expressions { nodes: children })
    }

    fn import_or_expression(&mut self, start: Token) -> ParseResult {
        if start.token_type == TokenType::Import {
            self.import(start)
        } else {
            self.expression(start)
        }
    }

    fn expression(&mut self, start: Token) -> ParseResult {
        self.binary_or(start)
    }

    fn binary_or(&mut self, start: Token) -> ParseResult {
        binary_op!(self, start, binary_and, Or)
    }

    fn binary_and(&mut self, start: Token) -> ParseResult {
        binary_op!(self, start, equality, And)
    }

    fn equality(&mut self, start: Token) -> ParseResult {
        binary_op!(self, start, compare, Equal, NotEqual)
    }

    fn compare(&mut self, start: Token) -> ParseResult {
        binary_op!(
            self,
            start,
            bitwise_or,
            Lower,
            LowerEqual,
            Greater,
            GreaterEqual
        )
    }

    fn bitwise_or(&mut self, start: Token) -> ParseResult {
        binary_op!(self, start, bitwise_and, BitwiseOr, BitwiseXor)
    }

    fn bitwise_and(&mut self, start: Token) -> ParseResult {
        binary_op!(self, start, bitwise_shift, BitwiseAnd)
    }

    fn bitwise_shift(&mut self, start: Token) -> ParseResult {
        binary_op!(self, start, add_subtract, ShiftLeft, ShiftRight)
    }

    fn add_subtract(&mut self, start: Token) -> ParseResult {
        binary_op!(self, start, div_mod_mul, Add, Sub)
    }

    fn div_mod_mul(&mut self, start: Token) -> ParseResult {
        binary_op!(self, start, pow, Div, Mod, Mul)
    }

    fn pow(&mut self, start: Token) -> ParseResult {
        binary_op!(self, start, range, Pow)
    }

    fn range(&mut self, start: Token) -> ParseResult {
        binary_op!(self, start, bracket_send, InclusiveRange, ExclusiveRange)
    }

    /// Parses an expression such as `X[Y]` and `X[Y] = Z`.
    fn bracket_send(&mut self, start: Token) -> ParseResult {
        let start_line = start.line;
        let mut node = self.type_cast(start)?;

        while self.lexer.next_type_is(&TokenType::BracketOpen) {
            // Only treat [x][y] as a send if [y] occurs on the same line. This
            // ensures that e.g. [x]\n[y] is parsed as two array literals.
            if self.lexer.peek().unwrap().line != start_line {
                break;
            }

            let bracket = self.lexer.next().unwrap();

            let (name, args) = self.bracket_get_or_set()?;

            node = Node::Send {
                name: name,
                receiver: Some(Box::new(node)),
                arguments: args,
                line: bracket.line,
                column: bracket.column,
            };
        }

        Ok(node)
    }

    /// Parses an expression such as `[X]` or `[X] = Y`.
    fn bracket_get_or_set(&mut self) -> Result<(String, Vec<Node>), String> {
        let mut args = Vec::new();

        while let Some(start) = self.lexer.next() {
            if start.token_type == TokenType::BracketClose {
                break;
            }

            args.push(self.expression(start)?);

            if self.lexer.next_type_is(&TokenType::Comma) {
                self.lexer.next();
            } else if let Some(next) = self.lexer.peek() {
                if next.token_type != TokenType::BracketClose {
                    parse_error!("Unexpected token {:?}", next.token_type);
                }
            } else {
                parse_error!("Unexpected end of input");
            }
        }

        if self.lexer.next_type_is(&TokenType::Assign) {
            self.lexer.next();

            let start = next_or_error!(self);

            args.push(self.expression(start)?);

            Ok(("[]=".to_string(), args))
        } else {
            Ok(("[]".to_string(), args))
        }
    }

    fn type_cast(&mut self, start: Token) -> ParseResult {
        let mut node = self.send_chain(start)?;

        if self.lexer.next_type_is(&TokenType::As) {
            let op = next_or_error!(self);
            let token = next_or_error!(self);
            let tname = self.type_name(token)?;

            node = Node::TypeCast {
                value: Box::new(node),
                target_type: Box::new(tname),
                line: op.line,
                column: op.column,
            };
        }

        Ok(node)
    }

    /// Parses a chain of messages being sent to a receiver.
    fn send_chain(&mut self, start: Token) -> ParseResult {
        let mut node = self.value(start)?;

        while self.lexer.next_type_is(&TokenType::Dot) {
            self.lexer.next();

            let (name, line, column) = self.send_name()?;
            let args = self.send_chain_arguments(line)?;

            node = Node::Send {
                name: name,
                receiver: Some(Box::new(node)),
                arguments: args,
                line: line,
                column: column,
            };
        }

        Ok(node)
    }

    /// Returns the name and position to use for sending a message to an object.
    fn send_name(&mut self) -> Result<(String, usize, usize), String> {
        let token = next_or_error!(self);
        let column = token.column;
        let line = token.line;

        Ok((self.message_name_for_token(token)?, line, column))
    }

    /// Parses the arguments for a method call part of a method call chain.
    fn send_chain_arguments(&mut self, line: usize) -> Result<Vec<Node>, String> {
        if self.lexer.next_type_is(&TokenType::ParenOpen) {
            self.arguments_with_parenthesis()
        } else {
            if self.next_expression_is_argument(line) {
                self.arguments_without_parenthesis()
            } else {
                Ok(Vec::new())
            }
        }
    }

    /// Parses a list of send arguments wrapped in parenthesis.
    ///
    /// Example:
    ///
    ///     (10, 'foo', 'bar')
    fn arguments_with_parenthesis(&mut self) -> Result<Vec<Node>, String> {
        let mut args = Vec::new();

        // Skip the opening parenthesis.
        self.lexer.next();

        while let Some(token) = self.lexer.next() {
            if token.token_type == TokenType::ParenClose {
                break;
            }

            args.push(self.send_argument(token)?);

            if self.lexer.next_type_is(&TokenType::Comma) {
                self.lexer.next();
            } else if let Some(token) = self.lexer.peek() {
                if token.token_type != TokenType::ParenClose {
                    parse_error!(
                        "Expected a comma, not a {:?}",
                        token.token_type
                    );
                }
            }
        }

        Ok(args)
    }

    /// Parses a list of send arguments without parenthesis.
    ///
    /// Example:
    ///
    ///     10, 'foo', 'bar'
    fn arguments_without_parenthesis(&mut self) -> Result<Vec<Node>, String> {
        let mut args = Vec::new();

        while let Some(token) = self.lexer.next() {
            args.push(self.send_argument(token)?);

            if self.lexer.next_type_is(&TokenType::Comma) {
                self.lexer.next();
            } else {
                break;
            }
        }

        Ok(args)
    }

    /// Parses an argument passed to a method call.
    ///
    /// Examples:
    ///
    ///     foo(10)
    ///     foo(number: 10)
    fn send_argument(&mut self, start: Token) -> ParseResult {
        if self.lexer.next_type_is(&TokenType::Colon) {
            self.lexer.next();

            let value = {
                let token = next_or_error!(self);

                self.expression(token)?
            };

            Ok(Node::KeywordArgument {
                name: start.value,
                value: Box::new(value),
                line: start.line,
                column: start.column,
            })
        } else {
            self.expression(start)
        }
    }

    fn value(&mut self, start: Token) -> ParseResult {
        match start.token_type {
            TokenType::String => self.string(start),
            TokenType::Integer => self.integer(start),
            TokenType::Float => self.float(start),
            TokenType::Identifier => self.identifier(start),
            TokenType::Constant => self.constant(start),
            TokenType::CurlyOpen => self.closure_without_arguments(start),
            TokenType::Sub => self.negative_number(start),
            TokenType::BracketOpen => self.array(start),
            TokenType::HashOpen => self.hash(start),
            TokenType::Function => self.method_or_closure(start),
            TokenType::Let => self.let_define(start),
            TokenType::Var => self.var_define(start),
            TokenType::Class => self.class(start),
            TokenType::Trait => self.def_trait(start),
            TokenType::Return => self.return_value(start),
            TokenType::Comment => self.comment(start),
            TokenType::Type => self.def_type(start),
            TokenType::Attribute => self.attribute(start),
            TokenType::SelfObject => self.self_object(start),
            TokenType::Throw => self.throw(start),
            TokenType::Try => self.try(start),
            _ => {
                parse_error!(
                    "An expression can not start with {:?}",
                    start.token_type
                )
            }
        }
    }

    /// Parses an identifier, or a method call on "self".
    ///
    /// An identifier can be followed by an argument list, either using
    /// parenthesis or without.
    ///
    /// Examples:
    ///
    ///     foo
    ///     foo(bar, baz)
    ///     foo bar, baz
    fn identifier(&mut self, start: Token) -> ParseResult {
        if self.lexer.next_type_is(&TokenType::Assign) {
            self.reassign_local(start)
        } else {
            send_or!(self, start, self.identifier_from_token(start))
        }
    }

    /// Parses an attribute
    ///
    /// Example:
    ///
    ///     @foo
    fn attribute(&mut self, start: Token) -> ParseResult {
        if self.lexer.next_type_is(&TokenType::Assign) {
            self.reassign_attribute(start)
        } else {
            Ok(self.attribute_from_token(start))
        }
    }

    /// Parses the reassignment of a local variable.
    fn reassign_local(&mut self, start: Token) -> ParseResult {
        self.lexer.next();

        let line = start.line;
        let column = start.column;
        let local = self.identifier_from_token(start);
        let value = {
            let token = next_or_error!(self);

            self.expression(token)?
        };

        Ok(Node::Reassign {
            variable: Box::new(local),
            value: Box::new(value),
            line: line,
            column: column,
        })
    }

    /// Parses the reassignment of an attribute.
    fn reassign_attribute(&mut self, start: Token) -> ParseResult {
        self.lexer.next();

        let line = start.line;
        let column = start.column;
        let attr = self.attribute_from_token(start);
        let value = {
            let token = next_or_error!(self);

            self.expression(token)?
        };

        Ok(Node::Reassign {
            variable: Box::new(attr),
            value: Box::new(value),
            line: line,
            column: column,
        })
    }

    /// Parses a constant.
    ///
    /// Examples:
    ///
    ///     Foo
    ///     Foo::Bar
    fn constant(&mut self, start: Token) -> ParseResult {
        let mut node = self.constant_from_token(start, None);

        while self.lexer.next_type_is(&TokenType::ColonColon) {
            self.lexer.next();

            let start = next_of_type!(self, TokenType::Constant);

            node = self.constant_from_token(start, Some(Box::new(node)));
        }

        Ok(node)
    }

    /// Parses a type name.
    ///
    /// Examples:
    ///
    ///     Foo
    ///     Foo!(Bar)
    ///     Foo::Bar!(Baz)
    fn type_name(&mut self, start: Token) -> ParseResult {
        let line = start.line;
        let column = start.column;
        let node = self.constant(start)?;
        let args = self.optional_type_arguments()?;
        let rtype = self.optional_return_type()?;

        Ok(Node::Type {
            constant: Box::new(node),
            arguments: args,
            return_type: rtype,
            line: line,
            column: column,
        })
    }

    fn type_name_or_union_type(&mut self, start: Token) -> ParseResult {
        let line = start.line;
        let col = start.column;
        let node = self.type_name(start)?;

        if self.lexer.next_type_is(&TokenType::BitwiseOr) {
            let mut types = vec![node];

            while self.lexer.next_type_is(&TokenType::BitwiseOr) {
                self.lexer.next();

                let start = next_or_error!(self);

                types.push(self.type_name(start)?);
            }

            Ok(Node::UnionType { types: types, line: line, column: col })
        } else {
            Ok(node)
        }
    }

    /// Parses a closure
    ///
    /// Examples:
    ///
    ///     fn { body }
    ///     fn(arg) { body }
    ///     fn(arg: T) { body }
    ///     fn(arg: T) -> T { body }
    fn closure(&mut self, start: Token) -> ParseResult {
        // Parse the arguments
        let args = self.optional_arguments()?;
        let ret_type = self.optional_return_type()?;

        next_of_type!(self, TokenType::CurlyOpen);

        let body = self.block()?;

        Ok(Node::Closure {
            arguments: args,
            return_type: ret_type,
            body: Box::new(body),
            line: start.line,
            column: start.column,
        })
    }

    /// Parses a closure without arguments.
    ///
    /// Examples:
    ///
    ///     { body }
    fn closure_without_arguments(&mut self, start: Token) -> ParseResult {
        let body = self.block()?;

        Ok(Node::Closure {
            arguments: Vec::new(),
            return_type: None,
            body: Box::new(body),
            line: start.line,
            column: start.column,
        })
    }

    /// Parses a list of a argument definitions.
    fn def_arguments(&mut self) -> Result<Vec<Node>, String> {
        let mut args = Vec::new();

        while self.lexer.peek().is_some() {
            // Break out early if we're dealing with an empty arguments list.
            if self.lexer.next_type_is(&TokenType::ParenClose) {
                self.lexer.next();
                break;
            }

            let rest = if self.lexer.next_type_is(&TokenType::Mul) {
                self.lexer.next();

                true
            } else {
                false
            };

            // Parse the argument's name and position.
            let (name, line, column) = if let Some(token) = self.lexer.next() {
                (token.value, token.line, token.column)
            } else {
                break;
            };

            // Parse the argument's type, if any.
            let arg_type = self.def_argument_type()?;

            // Parse the default value, if any.
            let default = if self.lexer.next_type_is(&TokenType::Assign) {
                self.lexer.next();

                let start = next_or_error!(self);

                Some(Box::new(self.expression(start)?))
            } else {
                None
            };

            args.push(Node::ArgumentDefine {
                name: name,
                value_type: arg_type,
                default: default,
                line: line,
                column: column,
                rest: rest,
            });

            comma_or_break_on!(self, TokenType::ParenClose);
        }

        Ok(args)
    }

    fn def_argument_type(&mut self) -> Result<Option<Box<Node>>, String> {
        if self.lexer.next_type_is(&TokenType::Colon) {
            self.lexer.next();

            let start = next_or_error!(self);
            let vtype = self.type_name_or_union_type(start)?;

            Ok(Some(Box::new(vtype)))
        } else {
            Ok(None)
        }
    }

    fn optional_type_arguments(&mut self) -> Result<Vec<Node>, String> {
        if self.lexer.next_type_is(&TokenType::TypeArgsOpen) {
            self.lexer.next();

            Ok(self.type_arguments()?)
        } else {
            Ok(Vec::new())
        }
    }

    /// Parses a list of type arguments.
    fn type_arguments(&mut self) -> Result<Vec<Node>, String> {
        let mut args = Vec::new();

        loop {
            let start = next_or_error!(self);
            let tname = self.type_name(start)?;

            args.push(tname);

            comma_or_break_on!(self, TokenType::ParenClose);
        }

        Ok(args)
    }

    fn string(&mut self, start: Token) -> ParseResult {
        Ok(Node::String {
            value: start.value,
            line: start.line,
            column: start.column,
        })
    }

    /// Parses a negative number such as -10 or -2.5.
    fn negative_number(&mut self, start: Token) -> ParseResult {
        let following = next_or_error!(self);

        match following.token_type {
            TokenType::Integer => {
                let val = -parse_value!(i64, following.value)?;

                Ok(Node::Integer {
                    value: val,
                    line: start.line,
                    column: start.column,
                })
            }
            TokenType::Float => {
                let val = -parse_value!(f64, following.value)?;

                Ok(Node::Float {
                    value: val,
                    line: start.line,
                    column: start.column,
                })
            }
            _ => {
                parse_error!(
                    "Unexpected token {:?}, expected a number",
                    following.token_type
                )
            }
        }
    }

    fn integer(&mut self, start: Token) -> ParseResult {
        let val = parse_value!(i64, start.value)?;

        Ok(Node::Integer {
            value: val,
            line: start.line,
            column: start.column,
        })
    }

    fn float(&mut self, start: Token) -> ParseResult {
        let val = parse_value!(f64, start.value)?;

        Ok(Node::Float {
            value: val,
            line: start.line,
            column: start.column,
        })
    }

    fn array(&mut self, start: Token) -> ParseResult {
        let mut values = Vec::new();

        loop {
            let expr_start = next_or_error!(self);

            if expr_start.token_type == TokenType::BracketClose {
                break;
            }

            values.push(self.expression(expr_start)?);

            comma_or_break_on!(self, TokenType::BracketClose);
        }

        Ok(Node::Array {
            values: values,
            line: start.line,
            column: start.column,
        })
    }

    /// Parses a hash map.
    ///
    /// Example:
    ///
    ///     %{ 'name': 'Alice', 'age': 42 }
    fn hash(&mut self, start: Token) -> ParseResult {
        let mut pairs = Vec::new();

        loop {
            let key_start = next_or_error!(self);

            if key_start.token_type == TokenType::CurlyClose {
                break;
            }

            let key = self.expression(key_start)?;

            next_of_type!(self, TokenType::Colon);

            let val_start = next_or_error!(self);
            let val = self.expression(val_start)?;

            pairs.push((key, val));

            comma_or_break_on!(self, TokenType::CurlyClose);
        }

        Ok(Node::Hash {
            pairs: pairs,
            line: start.line,
            column: start.column,
        })
    }

    /// Parses a method or closure definition
    fn method_or_closure(&mut self, start: Token) -> ParseResult {
        let is_closure = if let Some(token) = self.lexer.peek() {
            match token.token_type {
                TokenType::ParenOpen | TokenType::CurlyOpen |
                TokenType::Arrow => true,
                _ => false,
            }
        } else {
            parse_error!(
                "Expected \"fn\" to be followed by an identifier or \
                          an arguments list"
            );
        };

        if is_closure {
            self.closure(start)
        } else {
            self.def_method(start)
        }
    }

    /// Parses a method definition.
    fn def_method(&mut self, start: Token) -> ParseResult {
        let (receiver, name) = {
            let left = next_or_error!(self);

            // "def foo.bar" means "define bar on whatever foo refers to"
            if self.lexer.next_type_is(&TokenType::Dot) {
                // Receivers can only be single values, not fully fledged
                // expressions.
                let receiver = self.value(left)?;

                // Skip over the dot. We're using next_of_type! here to make
                // sure the dot isn't consumed by accident when parsing the
                // receiver.
                next_of_type!(self, TokenType::Dot);

                let right = next_or_error!(self);

                (
                    Some(Box::new(receiver)),
                    self.message_name_for_token(right)?,
                )
            } else {
                (None, self.message_name_for_token(left)?)
            }
        };

        let type_arguments = self.optional_type_arguments()?;
        let arguments = self.optional_arguments()?;
        let return_type = self.optional_return_type()?;
        let throw_type = self.optional_throw_type()?;

        let body = if self.lexer.next_type_is(&TokenType::CurlyOpen) {
            next_of_type!(self, TokenType::CurlyOpen);

            Some(Box::new(self.block()?))
        } else {
            None
        };

        Ok(Node::Method {
            receiver: receiver,
            name: name,
            arguments: arguments,
            type_arguments: type_arguments,
            return_type: return_type,
            throw_type: throw_type,
            body: body,
            line: start.line,
            column: start.column,
        })
    }

    /// Defines an immutable variable.
    fn let_define(&mut self, start: Token) -> ParseResult {
        let name = self.variable_name()?;
        let value_type = self.optional_variable_type()?;
        let value = self.variable_value()?;

        Ok(Node::LetDefine {
            name: Box::new(name),
            value: Box::new(value),
            value_type: value_type,
            line: start.line,
            column: start.column,
        })
    }

    /// Defines a mutable variable.
    fn var_define(&mut self, start: Token) -> ParseResult {
        let name = self.variable_name()?;
        let value_type = self.optional_variable_type()?;
        let value = self.variable_value()?;

        Ok(Node::VarDefine {
            name: Box::new(name),
            value: Box::new(value),
            value_type: value_type,
            line: start.line,
            column: start.column,
        })
    }

    fn variable_name(&mut self) -> Result<Node, String> {
        let start = next_or_error!(self);

        let name = match start.token_type {
            TokenType::Identifier => self.identifier_from_token(start),
            TokenType::Attribute => self.attribute_from_token(start),
            TokenType::Constant => self.constant_from_token(start, None),
            _ => {
                panic!(
                    "Unexpected {:?}, expected an identifier or attribute",
                    start.token_type
                )
            }
        };

        Ok(name)
    }

    fn optional_variable_type(&mut self) -> Result<Option<Box<Node>>, String> {
        let mut var_type = None;

        if self.lexer.next_type_is(&TokenType::Colon) {
            self.lexer.next();

            let start = next_of_type!(self, TokenType::Constant);
            let constant = self.constant(start)?;

            var_type = Some(Box::new(constant));
        }

        Ok(var_type)
    }

    fn variable_value(&mut self) -> Result<Node, String> {
        next_of_type!(self, TokenType::Assign);

        let start = next_or_error!(self);

        self.expression(start)
    }

    /// Parses a class definition.
    fn class(&mut self, start: Token) -> ParseResult {
        let name = next_of_type!(self, TokenType::Constant);
        let type_args = self.optional_type_arguments()?;
        let mut implements = Vec::new();

        while self.lexer.next_type_is(&TokenType::Impl) {
            let start = next_or_error!(self);

            implements.push(self.implement_trait(start)?);
        }

        next_of_type!(self, TokenType::CurlyOpen);

        let body = self.block()?;

        Ok(Node::Class {
            name: name.value,
            type_arguments: type_args,
            implements: implements,
            body: Box::new(body),
            line: start.line,
            column: start.column,
        })
    }

    /// Parses a trait definition.
    fn def_trait(&mut self, start: Token) -> ParseResult {
        let name = next_of_type!(self, TokenType::Constant);
        let type_args = self.optional_type_arguments()?;
        let body = self.block()?;

        Ok(Node::Trait {
            name: name.value,
            type_arguments: type_args,
            body: Box::new(body),
            line: start.line,
            column: start.column,
        })
    }

    /// Parses the "return" keyword
    fn return_value(&mut self, start: Token) -> ParseResult {
        let value = if self.next_expression_is_argument(start.line) {
            let next = self.lexer.next().unwrap();

            Some(Box::new(self.expression(next)?))
        } else {
            None
        };

        Ok(Node::Return {
            value: value,
            line: start.line,
            column: start.column,
        })
    }

    /// Parses the implementation of a trait.
    fn implement_trait(&mut self, start: Token) -> ParseResult {
        let name = {
            let token = next_or_error!(self);

            self.type_name(token)?
        };

        let type_args = self.optional_type_arguments()?;

        let renames = if self.lexer.next_type_is(&TokenType::ParenOpen) {
            self.lexer.next();

            self.trait_renames()?
        } else {
            Vec::new()
        };

        Ok(Node::Implement {
            name: Box::new(name),
            type_arguments: type_args,
            renames: renames,
            line: start.line,
            column: start.column,
        })
    }

    /// Parses a list of method renames for trait implementations.
    fn trait_renames(&mut self) -> Result<Vec<(Node, Node)>, String> {
        let mut renames = Vec::new();

        loop {
            let src_name = {
                let token = next_of_type!(self, TokenType::Identifier);

                self.identifier_from_token(token)
            };

            next_of_type!(self, TokenType::As);

            let new_name = {
                let token = next_of_type!(self, TokenType::Identifier);

                self.identifier_from_token(token)
            };

            renames.push((src_name, new_name));

            if self.lexer.next_type_is(&TokenType::Comma) {
                self.lexer.next();
            } else {
                break;
            }
        }

        next_of_type!(self, TokenType::ParenClose);

        Ok(renames)
    }

    /// Parses a comment
    fn comment(&mut self, start: Token) -> ParseResult {
        Ok(Node::Comment {
            value: start.value,
            line: start.line,
            column: start.column,
        })
    }

    /// Parses an import statement.
    ///
    /// Examples:
    ///
    ///     import foo::bar
    ///     import foo::bar::Baz
    ///     import foo::bar::(Baz, Quix as Foo)
    fn import(&mut self, start: Token) -> ParseResult {
        let mut steps = Vec::new();
        let mut symbols = Vec::new();

        loop {
            let step = next_or_error!(self);

            match step.token_type {
                TokenType::Identifier => {
                    steps.push(self.identifier_from_token(step));
                }
                TokenType::Constant => {
                    // We're importing a single constant, without an alias.
                    symbols.push(Node::ImportSymbol {
                        symbol: Box::new(self.constant_from_token(step, None)),
                        alias: None,
                    });

                    break;
                }
                _ => {
                    parse_error!(
                        "Unexpected {:?}, expected an identifier or \
                                  constant",
                        step.token_type
                    );
                }
            }

            if self.lexer.next_type_is(&TokenType::ColonColon) {
                self.lexer.next();
            } else {
                break;
            }

            if self.lexer.next_type_is(&TokenType::ParenOpen) {
                self.lexer.next();
                symbols = self.import_symbols()?;
                break;
            }
        }

        Ok(Node::Import {
            steps: steps,
            symbols: symbols,
            line: start.line,
            column: start.column,
        })
    }

    /// Parses a list of symbols and aliases to import.
    fn import_symbols(&mut self) -> Result<Vec<Node>, String> {
        let mut symbols = Vec::new();

        loop {
            let start = next_of_type!(self, TokenType::Constant);
            let symbol = self.constant_from_token(start, None);

            let alias = if self.lexer.next_type_is(&TokenType::As) {
                self.lexer.next();

                let start = next_of_type!(self, TokenType::Constant);

                Some(Box::new(self.constant_from_token(start, None)))
            } else {
                None
            };

            symbols.push(Node::ImportSymbol {
                symbol: Box::new(symbol),
                alias: alias,
            });

            comma_or_break_on!(self, TokenType::ParenClose);
        }

        Ok(symbols)
    }

    /// Parses a type alias definition.
    ///
    /// Example:
    ///
    ///     type MyResult!(T) = Result!(T, String)
    fn def_type(&mut self, start: Token) -> ParseResult {
        let name = {
            let token = next_or_error!(self);

            self.type_name(token)?
        };

        next_of_type!(self, TokenType::Assign);

        let value = {
            let token = next_or_error!(self);

            self.type_name_or_union_type(token)?
        };

        Ok(Node::TypeDefine {
            name: Box::new(name),
            value: Box::new(value),
            line: start.line,
            column: start.column,
        })
    }

    /// Parses a block of code
    ///
    /// Example:
    ///
    ///     { 10 }
    fn block(&mut self) -> Result<Node, String> {
        let mut body = Vec::new();

        while let Some(token) = self.lexer.next() {
            if token.token_type == TokenType::CurlyClose {
                break;
            }

            body.push(self.expression(token)?);
        }

        Ok(Node::Expressions { nodes: body })
    }

    fn block_with_optional_curly_braces(&mut self) -> Result<Node, String> {
        if self.lexer.next_type_is(&TokenType::CurlyOpen) {
            next_or_error!(self);

            self.block()
        } else {
            let start = next_or_error!(self);

            Ok(Node::Expressions { nodes: vec![self.expression(start)?] })
        }
    }

    fn self_object(&mut self, start: Token) -> ParseResult {
        Ok(Node::SelfObject { line: start.line, column: start.column })
    }

    /// Parses the "try" keyword.
    fn try(&mut self, start: Token) -> ParseResult {
        let body = self.block_with_optional_curly_braces()?;

        let (else_arg, else_body) = if self.lexer.next_type_is(
            &TokenType::Else,
        ) {
            next_or_error!(self);

            let else_arg = self.optional_else_arg()?;
            let else_body = Box::new(self.block_with_optional_curly_braces()?);

            (else_arg, Some(else_body))
        } else {
            (None, None)
        };

        Ok(Node::Try {
            body: Box::new(body),
            else_body: else_body,
            else_argument: else_arg,
            line: start.line,
            column: start.column,
        })
    }

    /// Parses an optional argument for the "else" keyword.
    fn optional_else_arg(&mut self) -> Result<Option<Box<Node>>, String> {
        if self.lexer.next_type_is(&TokenType::ParenOpen) {
            // Consume the opening parenthesis
            next_or_error!(self);

            let name = next_of_type!(self, TokenType::Identifier);

            // Consume the closing parenthesis
            next_of_type!(self, TokenType::ParenClose);

            Ok(Some(Box::new(self.identifier_from_token(name))))
        } else {
            Ok(None)
        }
    }

    /// Parses the "throw" keyword.
    fn throw(&mut self, start: Token) -> ParseResult {
        let expr_start = next_or_error!(self);
        let value = self.expression(expr_start)?;

        Ok(Node::Throw {
            value: Box::new(value),
            line: start.line,
            column: start.column,
        })
    }

    fn optional_arguments(&mut self) -> Result<Vec<Node>, String> {
        if self.lexer.next_type_is(&TokenType::ParenOpen) {
            self.lexer.next();
            Ok(self.def_arguments()?)
        } else {
            Ok(Vec::new())
        }
    }

    fn optional_return_type(&mut self) -> Result<Option<Box<Node>>, String> {
        if self.lexer.next_type_is(&TokenType::Arrow) {
            self.lexer.next();

            let start = next_or_error!(self);
            let ret = self.type_name_or_union_type(start)?;

            Ok(Some(Box::new(ret)))
        } else {
            Ok(None)
        }
    }

    fn optional_throw_type(&mut self) -> Result<Option<Box<Node>>, String> {
        if self.lexer.next_type_is(&TokenType::Throw) {
            self.lexer.next();

            let start = next_or_error!(self);
            let ret = self.type_name_or_union_type(start)?;

            Ok(Some(Box::new(ret)))
        } else {
            Ok(None)
        }
    }

    /// Returns the name for a message for the given token, if any.
    fn message_name_for_token(&mut self, start: Token) -> Result<String, String> {
        if self.message_tokens.contains(&start.token_type) {
            let mut name = start.value;

            if start.token_type == TokenType::BracketOpen {
                next_of_type!(self, TokenType::BracketClose);

                name.push(']');
            }

            if self.lexer.next_type_is(&TokenType::Assign) {
                self.lexer.next();

                name.push('=');
            }

            Ok(name)
        } else {
            parse_error!(
                "Tokens of type {:?} are not valid for method names",
                start.token_type
            )
        }
    }

    fn identifier_from_token(&self, token: Token) -> Node {
        Node::Identifier {
            name: token.value,
            line: token.line,
            column: token.column,
        }
    }

    fn attribute_from_token(&self, token: Token) -> Node {
        Node::Attribute {
            name: token.value,
            line: token.line,
            column: token.column,
        }
    }

    fn constant_from_token(&self,
                           token: Token,
                           receiver: Option<Box<Node>>)
                           -> Node {
        Node::Constant {
            receiver: receiver,
            name: token.value,
            line: token.line,
            column: token.column,
        }
    }

    fn next_expression_is_argument(&mut self, current_line: usize) -> bool {
        if let Some(token) = self.lexer.peek() {
            self.value_start.contains(&token.token_type) &&
            token.line == current_line
        } else {
            false
        }
    }
}

use std::fmt::Write;

use crate::parsing::{
    ast::types::{
        ArrayExpression, ArrayRangeExpression, BinaryExpression, BinaryOperator, BinaryPart, BodyItem, CallExpression,
        CallExpressionKw, CommentStyle, DefaultParamVal, Expr, FnArgType, FormatOptions, FunctionExpression,
        IfExpression, ImportSelector, ImportStatement, ItemVisibility, LabeledArg, Literal, LiteralIdentifier,
        LiteralValue, MemberExpression, MemberObject, Node, NonCodeNode, NonCodeValue, ObjectExpression, Parameter,
        PipeExpression, Program, TagDeclarator, UnaryExpression, VariableDeclaration, VariableKind,
    },
    PIPE_OPERATOR,
};

impl Program {
    pub fn recast(&self, options: &FormatOptions, indentation_level: usize) -> String {
        let indentation = options.get_indentation(indentation_level);

        let result = self
            .shebang
            .as_ref()
            .map(|sh| format!("{}\n\n", sh.inner.content))
            .unwrap_or_default();

        let result = self
            .body
            .iter()
            .map(|body_item| match body_item.clone() {
                BodyItem::ImportStatement(stmt) => stmt.recast(options, indentation_level),
                BodyItem::ExpressionStatement(expression_statement) => {
                    expression_statement
                        .expression
                        .recast(options, indentation_level, ExprContext::Other)
                }
                BodyItem::VariableDeclaration(variable_declaration) => {
                    variable_declaration.recast(options, indentation_level)
                }
                BodyItem::ReturnStatement(return_statement) => {
                    format!(
                        "{}return {}",
                        indentation,
                        return_statement
                            .argument
                            .recast(options, indentation_level, ExprContext::Other)
                            .trim_start()
                    )
                }
            })
            .enumerate()
            .fold(result, |mut output, (index, recast_str)| {
                let start_string = if index == 0 {
                    // We need to indent.
                    if self.non_code_meta.start_nodes.is_empty() {
                        indentation.to_string()
                    } else {
                        self.non_code_meta
                            .start_nodes
                            .iter()
                            .map(|start| start.recast(options, indentation_level))
                            .collect()
                    }
                } else {
                    // Do nothing, we already applied the indentation elsewhere.
                    String::new()
                };

                // determine the value of the end string
                // basically if we are inside a nested function we want to end with a new line
                let maybe_line_break: String = if index == self.body.len() - 1 && indentation_level == 0 {
                    String::new()
                } else {
                    "\n".to_string()
                };

                let custom_white_space_or_comment = match self.non_code_meta.non_code_nodes.get(&index) {
                    Some(noncodes) => noncodes
                        .iter()
                        .enumerate()
                        .map(|(i, custom_white_space_or_comment)| {
                            let formatted = custom_white_space_or_comment.recast(options, indentation_level);
                            if i == 0 && !formatted.trim().is_empty() {
                                if let NonCodeValue::BlockComment { .. } = custom_white_space_or_comment.value {
                                    format!("\n{}", formatted)
                                } else {
                                    formatted
                                }
                            } else {
                                formatted
                            }
                        })
                        .collect::<String>(),
                    None => String::new(),
                };
                let end_string = if custom_white_space_or_comment.is_empty() {
                    maybe_line_break
                } else {
                    custom_white_space_or_comment
                };

                let _ = write!(output, "{}{}{}", start_string, recast_str, end_string);
                output
            })
            .trim()
            .to_string();

        // Insert a final new line if the user wants it.
        if options.insert_final_newline && !result.is_empty() {
            format!("{}\n", result)
        } else {
            result
        }
    }
}

impl NonCodeValue {
    fn should_cause_array_newline(&self) -> bool {
        match self {
            Self::InlineComment { .. } => false,
            Self::BlockComment { .. } | Self::NewLineBlockComment { .. } | Self::NewLine | Self::Annotation { .. } => {
                true
            }
        }
    }
}

impl Node<NonCodeNode> {
    fn recast(&self, options: &FormatOptions, indentation_level: usize) -> String {
        let indentation = options.get_indentation(indentation_level);
        match &self.value {
            NonCodeValue::InlineComment {
                value,
                style: CommentStyle::Line,
            } => format!(" // {}\n", value),
            NonCodeValue::InlineComment {
                value,
                style: CommentStyle::Block,
            } => format!(" /* {} */", value),
            NonCodeValue::BlockComment { value, style } => match style {
                CommentStyle::Block => format!("{}/* {} */", indentation, value),
                CommentStyle::Line => {
                    if value.trim().is_empty() {
                        format!("{}//\n", indentation)
                    } else {
                        format!("{}// {}\n", indentation, value.trim())
                    }
                }
            },
            NonCodeValue::NewLineBlockComment { value, style } => {
                let add_start_new_line = if self.start == 0 { "" } else { "\n\n" };
                match style {
                    CommentStyle::Block => format!("{}{}/* {} */\n", add_start_new_line, indentation, value),
                    CommentStyle::Line => {
                        if value.trim().is_empty() {
                            format!("{}{}//\n", add_start_new_line, indentation)
                        } else {
                            format!("{}{}// {}\n", add_start_new_line, indentation, value.trim())
                        }
                    }
                }
            }
            NonCodeValue::NewLine => "\n\n".to_string(),
            NonCodeValue::Annotation { name, properties } => {
                let mut result = "@".to_owned();
                result.push_str(&name.name);
                if let Some(properties) = properties {
                    result.push('(');
                    result.push_str(
                        &properties
                            .iter()
                            .map(|prop| {
                                format!(
                                    "{} = {}",
                                    prop.key.name,
                                    prop.value
                                        .recast(options, indentation_level + 1, ExprContext::Other)
                                        .trim()
                                )
                            })
                            .collect::<Vec<String>>()
                            .join(", "),
                    );
                    result.push(')');
                    result.push('\n');
                }

                result
            }
        }
    }
}

impl ImportStatement {
    pub fn recast(&self, options: &FormatOptions, indentation_level: usize) -> String {
        let indentation = options.get_indentation(indentation_level);
        let vis = if self.visibility == ItemVisibility::Export {
            "export "
        } else {
            ""
        };
        let mut string = format!("{}{}import ", vis, indentation);
        match &self.selector {
            ImportSelector::List { items } => {
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        string.push_str(", ");
                    }
                    string.push_str(&item.name.name);
                    if let Some(alias) = &item.alias {
                        // If the alias is the same, don't output it.
                        if item.name.name != alias.name {
                            string.push_str(&format!(" as {}", alias.name));
                        }
                    }
                }
                string.push_str(" from ");
            }
            ImportSelector::Glob(_) => string.push_str("* from "),
            ImportSelector::None { .. } => {}
        }
        string.push_str(&format!("\"{}\"", self.path));

        if let ImportSelector::None { alias: Some(alias) } = &self.selector {
            string.push_str(" as ");
            string.push_str(&alias.name);
        }
        string
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum ExprContext {
    Pipe,
    Decl,
    Other,
}

impl Expr {
    pub(crate) fn recast(&self, options: &FormatOptions, indentation_level: usize, mut ctxt: ExprContext) -> String {
        let is_decl = matches!(ctxt, ExprContext::Decl);
        if is_decl {
            // Just because this expression is being bound to a variable, doesn't mean that every child
            // expression is being bound. So, reset the expression context if necessary.
            // This will still preserve the "::Pipe" context though.
            ctxt = ExprContext::Other;
        }
        match &self {
            Expr::BinaryExpression(bin_exp) => bin_exp.recast(options),
            Expr::ArrayExpression(array_exp) => array_exp.recast(options, indentation_level, ctxt),
            Expr::ArrayRangeExpression(range_exp) => range_exp.recast(options, indentation_level, ctxt),
            Expr::ObjectExpression(ref obj_exp) => obj_exp.recast(options, indentation_level, ctxt),
            Expr::MemberExpression(mem_exp) => mem_exp.recast(),
            Expr::Literal(literal) => literal.recast(),
            Expr::FunctionExpression(func_exp) => {
                let mut result = if is_decl { String::new() } else { "fn".to_owned() };
                result += &func_exp.recast(options, indentation_level);
                result
            }
            Expr::CallExpression(call_exp) => call_exp.recast(options, indentation_level, ctxt),
            Expr::CallExpressionKw(call_exp) => call_exp.recast(options, indentation_level, ctxt),
            Expr::Identifier(ident) => ident.name.to_string(),
            Expr::TagDeclarator(tag) => tag.recast(),
            Expr::PipeExpression(pipe_exp) => pipe_exp.recast(options, indentation_level),
            Expr::UnaryExpression(unary_exp) => unary_exp.recast(options),
            Expr::IfExpression(e) => e.recast(options, indentation_level, ctxt),
            Expr::PipeSubstitution(_) => crate::parsing::PIPE_SUBSTITUTION_OPERATOR.to_string(),
            Expr::LabelledExpression(e) => {
                let mut result = e.expr.recast(options, indentation_level, ctxt);
                result += " as ";
                result += &e.label.name;
                result
            }
            Expr::None(_) => {
                unimplemented!("there is no literal None, see https://github.com/KittyCAD/modeling-app/issues/1115")
            }
        }
    }
}

impl BinaryPart {
    fn recast(&self, options: &FormatOptions, indentation_level: usize) -> String {
        match &self {
            BinaryPart::Literal(literal) => literal.recast(),
            BinaryPart::Identifier(identifier) => identifier.name.to_string(),
            BinaryPart::BinaryExpression(binary_expression) => binary_expression.recast(options),
            BinaryPart::CallExpression(call_expression) => {
                call_expression.recast(options, indentation_level, ExprContext::Other)
            }
            BinaryPart::CallExpressionKw(call_expression) => {
                call_expression.recast(options, indentation_level, ExprContext::Other)
            }
            BinaryPart::UnaryExpression(unary_expression) => unary_expression.recast(options),
            BinaryPart::MemberExpression(member_expression) => member_expression.recast(),
            BinaryPart::IfExpression(e) => e.recast(options, indentation_level, ExprContext::Other),
        }
    }
}

impl CallExpression {
    fn recast(&self, options: &FormatOptions, indentation_level: usize, ctxt: ExprContext) -> String {
        format!(
            "{}{}({})",
            if ctxt == ExprContext::Pipe {
                "".to_string()
            } else {
                options.get_indentation(indentation_level)
            },
            self.callee.name,
            self.arguments
                .iter()
                .map(|arg| arg.recast(options, indentation_level, ctxt))
                .collect::<Vec<String>>()
                .join(", ")
        )
    }
}

impl CallExpressionKw {
    fn recast(&self, options: &FormatOptions, indentation_level: usize, ctxt: ExprContext) -> String {
        let indent = if ctxt == ExprContext::Pipe {
            "".to_string()
        } else {
            options.get_indentation(indentation_level)
        };
        let name = &self.callee.name;
        let mut arg_list = if let Some(first_arg) = &self.unlabeled {
            vec![first_arg.recast(options, indentation_level, ctxt)]
        } else {
            Vec::new()
        };
        arg_list.extend(
            self.arguments
                .iter()
                .map(|arg| arg.recast(options, indentation_level, ctxt)),
        );
        let args = arg_list.join(", ");
        format!("{indent}{name}({args})")
    }
}

impl LabeledArg {
    fn recast(&self, options: &FormatOptions, indentation_level: usize, ctxt: ExprContext) -> String {
        let label = &self.label.name;
        let arg = self.arg.recast(options, indentation_level, ctxt);
        format!("{label} = {arg}")
    }
}

impl VariableDeclaration {
    pub fn recast(&self, options: &FormatOptions, indentation_level: usize) -> String {
        let indentation = options.get_indentation(indentation_level);
        let mut output = match self.visibility {
            ItemVisibility::Default => String::new(),
            ItemVisibility::Export => "export ".to_owned(),
        };

        let (keyword, eq) = match self.kind {
            VariableKind::Fn => ("fn ", ""),
            VariableKind::Const => ("", " = "),
        };
        let _ = write!(
            output,
            "{}{keyword}{}{eq}{}",
            indentation,
            self.declaration.id.name,
            self.declaration
                .init
                .recast(options, indentation_level, ExprContext::Decl)
                .trim()
        );
        output
    }
}

impl Literal {
    fn recast(&self) -> String {
        match self.value {
            LiteralValue::Number(x) => {
                if self.raw.contains('.') && x.fract() == 0.0 {
                    format!("{x:?}")
                } else {
                    self.raw.clone()
                }
            }
            LiteralValue::String(ref s) => {
                let quote = if self.raw.trim().starts_with('"') { '"' } else { '\'' };
                format!("{quote}{s}{quote}")
            }
            LiteralValue::Bool(_) => self.raw.clone(),
        }
    }
}

impl TagDeclarator {
    pub fn recast(&self) -> String {
        // TagDeclarators are always prefixed with a dollar sign.
        format!("${}", self.name)
    }
}

impl ArrayExpression {
    fn recast(&self, options: &FormatOptions, indentation_level: usize, ctxt: ExprContext) -> String {
        // Reconstruct the order of items in the array.
        // An item can be an element (i.e. an expression for a KCL value),
        // or a non-code item (e.g. a comment)
        let num_items = self.elements.len() + self.non_code_meta.non_code_nodes_len();
        let mut elems = self.elements.iter();
        let mut found_line_comment = false;
        let mut format_items: Vec<_> = (0..num_items)
            .flat_map(|i| {
                if let Some(noncode) = self.non_code_meta.non_code_nodes.get(&i) {
                    noncode
                        .iter()
                        .map(|nc| {
                            found_line_comment |= nc.value.should_cause_array_newline();
                            nc.recast(options, 0)
                        })
                        .collect::<Vec<_>>()
                } else {
                    let el = elems.next().unwrap();
                    let s = format!("{}, ", el.recast(options, 0, ExprContext::Other));
                    vec![s]
                }
            })
            .collect();

        // Format these items into a one-line array.
        if let Some(item) = format_items.last_mut() {
            if let Some(norm) = item.strip_suffix(", ") {
                *item = norm.to_owned();
            }
        }
        let format_items = format_items; // Remove mutability
        let flat_recast = format!("[{}]", format_items.join(""));

        // We might keep the one-line representation, if it's short enough.
        let max_array_length = 40;
        let multi_line = flat_recast.len() > max_array_length || found_line_comment;
        if !multi_line {
            return flat_recast;
        }

        // Otherwise, we format a multi-line representation.
        let inner_indentation = if ctxt == ExprContext::Pipe {
            options.get_indentation_offset_pipe(indentation_level + 1)
        } else {
            options.get_indentation(indentation_level + 1)
        };
        let formatted_array_lines = format_items
            .iter()
            .map(|s| {
                format!(
                    "{inner_indentation}{}{}",
                    if let Some(x) = s.strip_suffix(" ") { x } else { s },
                    if s.ends_with('\n') { "" } else { "\n" }
                )
            })
            .collect::<Vec<String>>()
            .join("")
            .to_owned();
        let end_indent = if ctxt == ExprContext::Pipe {
            options.get_indentation_offset_pipe(indentation_level)
        } else {
            options.get_indentation(indentation_level)
        };
        format!("[\n{formatted_array_lines}{end_indent}]")
    }
}

/// An expression is syntactically trivial: i.e., a literal, identifier, or similar.
fn expr_is_trivial(expr: &Expr) -> bool {
    match expr {
        Expr::Literal(_) | Expr::Identifier(_) | Expr::TagDeclarator(_) | Expr::PipeSubstitution(_) | Expr::None(_) => {
            true
        }
        Expr::BinaryExpression(_)
        | Expr::FunctionExpression(_)
        | Expr::CallExpression(_)
        | Expr::CallExpressionKw(_)
        | Expr::PipeExpression(_)
        | Expr::ArrayExpression(_)
        | Expr::ArrayRangeExpression(_)
        | Expr::ObjectExpression(_)
        | Expr::MemberExpression(_)
        | Expr::UnaryExpression(_)
        | Expr::IfExpression(_)
        | Expr::LabelledExpression(_) => false,
    }
}

impl ArrayRangeExpression {
    fn recast(&self, options: &FormatOptions, _: usize, _: ExprContext) -> String {
        let s1 = self.start_element.recast(options, 0, ExprContext::Other);
        let s2 = self.end_element.recast(options, 0, ExprContext::Other);

        // Format these items into a one-line array. Put spaces around the `..` if either expression
        // is non-trivial. This is a bit arbitrary but people seem to like simple ranges to be formatted
        // tightly, but this is a misleading visual representation of the precedence if the range
        // components are compound expressions.
        if expr_is_trivial(&self.start_element) && expr_is_trivial(&self.end_element) {
            format!("[{s1}..{s2}]")
        } else {
            format!("[{s1} .. {s2}]")
        }

        // Assume a range expression fits on one line.
    }
}

impl ObjectExpression {
    fn recast(&self, options: &FormatOptions, indentation_level: usize, ctxt: ExprContext) -> String {
        if self
            .non_code_meta
            .non_code_nodes
            .values()
            .any(|nc| nc.iter().any(|nc| nc.value.should_cause_array_newline()))
        {
            return self.recast_multi_line(options, indentation_level, ctxt);
        }
        let flat_recast = format!(
            "{{ {} }}",
            self.properties
                .iter()
                .map(|prop| {
                    format!(
                        "{} = {}",
                        prop.key.name,
                        prop.value.recast(options, indentation_level + 1, ctxt).trim()
                    )
                })
                .collect::<Vec<String>>()
                .join(", ")
        );
        let max_array_length = 40;
        let needs_multiple_lines = flat_recast.len() > max_array_length;
        if !needs_multiple_lines {
            return flat_recast;
        }
        self.recast_multi_line(options, indentation_level, ctxt)
    }

    /// Recast, but always outputs the object with newlines between each property.
    fn recast_multi_line(&self, options: &FormatOptions, indentation_level: usize, ctxt: ExprContext) -> String {
        let inner_indentation = if ctxt == ExprContext::Pipe {
            options.get_indentation_offset_pipe(indentation_level + 1)
        } else {
            options.get_indentation(indentation_level + 1)
        };
        let num_items = self.properties.len() + self.non_code_meta.non_code_nodes_len();
        let mut props = self.properties.iter();
        let format_items: Vec<_> = (0..num_items)
            .flat_map(|i| {
                if let Some(noncode) = self.non_code_meta.non_code_nodes.get(&i) {
                    noncode.iter().map(|nc| nc.recast(options, 0)).collect::<Vec<_>>()
                } else {
                    let prop = props.next().unwrap();
                    // Use a comma unless it's the last item
                    let comma = if i == num_items - 1 { "" } else { ",\n" };
                    let s = format!(
                        "{} = {}{comma}",
                        prop.key.name,
                        prop.value.recast(options, indentation_level + 1, ctxt).trim()
                    );
                    vec![s]
                }
            })
            .collect();
        let end_indent = if ctxt == ExprContext::Pipe {
            options.get_indentation_offset_pipe(indentation_level)
        } else {
            options.get_indentation(indentation_level)
        };
        format!(
            "{{\n{inner_indentation}{}\n{end_indent}}}",
            format_items.join(&inner_indentation),
        )
    }
}

impl MemberExpression {
    fn recast(&self) -> String {
        let key_str = match &self.property {
            LiteralIdentifier::Identifier(identifier) => {
                if self.computed {
                    format!("[{}]", &(*identifier.name))
                } else {
                    format!(".{}", &(*identifier.name))
                }
            }
            LiteralIdentifier::Literal(lit) => format!("[{}]", &(*lit.raw)),
        };

        match &self.object {
            MemberObject::MemberExpression(member_exp) => member_exp.recast() + key_str.as_str(),
            MemberObject::Identifier(identifier) => identifier.name.to_string() + key_str.as_str(),
        }
    }
}

impl BinaryExpression {
    fn recast(&self, options: &FormatOptions) -> String {
        let maybe_wrap_it = |a: String, doit: bool| -> String {
            if doit {
                format!("({})", a)
            } else {
                a
            }
        };

        let should_wrap_right = match &self.right {
            BinaryPart::BinaryExpression(bin_exp) => {
                self.precedence() > bin_exp.precedence()
                    || self.operator == BinaryOperator::Sub
                    || self.operator == BinaryOperator::Div
            }
            _ => false,
        };

        let should_wrap_left = match &self.left {
            BinaryPart::BinaryExpression(bin_exp) => self.precedence() > bin_exp.precedence(),
            _ => false,
        };

        format!(
            "{} {} {}",
            maybe_wrap_it(self.left.recast(options, 0), should_wrap_left),
            self.operator,
            maybe_wrap_it(self.right.recast(options, 0), should_wrap_right)
        )
    }
}

impl UnaryExpression {
    fn recast(&self, options: &FormatOptions) -> String {
        match self.argument {
            BinaryPart::Literal(_)
            | BinaryPart::Identifier(_)
            | BinaryPart::MemberExpression(_)
            | BinaryPart::IfExpression(_)
            | BinaryPart::CallExpressionKw(_)
            | BinaryPart::CallExpression(_) => {
                format!("{}{}", &self.operator, self.argument.recast(options, 0))
            }
            BinaryPart::BinaryExpression(_) | BinaryPart::UnaryExpression(_) => {
                format!("{}({})", &self.operator, self.argument.recast(options, 0))
            }
        }
    }
}

impl IfExpression {
    fn recast(&self, options: &FormatOptions, indentation_level: usize, ctxt: ExprContext) -> String {
        // We can calculate how many lines this will take, so let's do it and avoid growing the vec.
        // Total lines = starting lines, else-if lines, ending lines.
        let n = 2 + (self.else_ifs.len() * 2) + 3;
        let mut lines = Vec::with_capacity(n);

        let cond = self.cond.recast(options, indentation_level, ctxt);
        lines.push((0, format!("if {cond} {{")));
        lines.push((1, self.then_val.recast(options, indentation_level + 1)));
        for else_if in &self.else_ifs {
            let cond = else_if.cond.recast(options, indentation_level, ctxt);
            lines.push((0, format!("}} else if {cond} {{")));
            lines.push((1, else_if.then_val.recast(options, indentation_level + 1)));
        }
        lines.push((0, "} else {".to_owned()));
        lines.push((1, self.final_else.recast(options, indentation_level + 1)));
        lines.push((0, "}".to_owned()));
        lines
            .into_iter()
            .map(|(ind, line)| format!("{}{}", options.get_indentation(indentation_level + ind), line.trim()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Node<PipeExpression> {
    fn recast(&self, options: &FormatOptions, indentation_level: usize) -> String {
        let pipe = self
            .body
            .iter()
            .enumerate()
            .map(|(index, statement)| {
                let indentation = options.get_indentation(indentation_level + 1);
                let mut s = statement.recast(options, indentation_level + 1, ExprContext::Pipe);
                let non_code_meta = self.non_code_meta.clone();
                if let Some(non_code_meta_value) = non_code_meta.non_code_nodes.get(&index) {
                    for val in non_code_meta_value {
                        let formatted = if val.end == self.end {
                            val.recast(options, indentation_level)
                                .trim_end_matches('\n')
                                .to_string()
                        } else {
                            val.recast(options, indentation_level + 1)
                                .trim_end_matches('\n')
                                .to_string()
                        };
                        if let NonCodeValue::BlockComment { .. } = val.value {
                            s += "\n";
                            s += &formatted;
                        } else {
                            s += &formatted;
                        }
                    }
                }

                if index != self.body.len() - 1 {
                    s += "\n";
                    s += &indentation;
                    s += PIPE_OPERATOR;
                    s += " ";
                }
                s
            })
            .collect::<String>();
        format!("{}{}", options.get_indentation(indentation_level), pipe)
    }
}

impl FunctionExpression {
    pub fn recast(&self, options: &FormatOptions, indentation_level: usize) -> String {
        // We don't want to end with a new line inside nested functions.
        let mut new_options = options.clone();
        new_options.insert_final_newline = false;
        let param_list = self
            .params
            .iter()
            .map(|param| param.recast(options, indentation_level))
            .collect::<Vec<String>>()
            .join(", ");
        let tab0 = options.get_indentation(indentation_level);
        let tab1 = options.get_indentation(indentation_level + 1);
        let return_type = match &self.return_type {
            Some(rt) => format!(": {}", rt.recast(&new_options, indentation_level)),
            None => String::new(),
        };
        let body = self.body.recast(&new_options, indentation_level + 1);

        format!("({param_list}){return_type} {{\n{tab1}{body}\n{tab0}}}")
    }
}

impl Parameter {
    pub fn recast(&self, options: &FormatOptions, indentation_level: usize) -> String {
        let at_sign = if self.labeled { "" } else { "@" };
        let identifier = &self.identifier.name;
        let question_mark = if self.default_value.is_some() { "?" } else { "" };
        let mut result = format!("{at_sign}{identifier}{question_mark}");
        if let Some(ty) = &self.type_ {
            result += ": ";
            result += &ty.recast(options, indentation_level);
        }
        if let Some(DefaultParamVal::Literal(ref literal)) = self.default_value {
            let lit = literal.recast();
            result.push_str(&format!(" = {lit}"));
        };

        result
    }
}

impl FnArgType {
    pub fn recast(&self, options: &FormatOptions, indentation_level: usize) -> String {
        match self {
            FnArgType::Primitive(t) => t.to_string(),
            FnArgType::Array(t) => format!("{t}[]"),
            FnArgType::Object { properties } => {
                let mut result = "{".to_owned();
                for p in properties {
                    result += " ";
                    result += &p.recast(options, indentation_level);
                    result += ",";
                }

                if result.ends_with(',') {
                    result.pop();
                    result += " ";
                }
                result += "}";

                result
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{parsing::ast::types::FormatOptions, source_range::ModuleId};

    #[test]
    fn test_recast_if_else_if_same() {
        let input = r#"b = if false {
  3
} else if true {
  4
} else {
  5
}
"#;
        let program = crate::parsing::top_level_parse(input).unwrap();
        let output = program.recast(&Default::default(), 0);
        assert_eq!(output, input);
    }

    #[test]
    fn test_recast_if_same() {
        let input = r#"b = if false {
  3
} else {
  5
}
"#;
        let program = crate::parsing::top_level_parse(input).unwrap();
        let output = program.recast(&Default::default(), 0);
        assert_eq!(output, input);
    }

    #[test]
    fn test_recast_import() {
        let input = r#"import a from "a.kcl"
import a as aaa from "a.kcl"
import a, b from "a.kcl"
import a as aaa, b from "a.kcl"
import a, b as bbb from "a.kcl"
import a as aaa, b as bbb from "a.kcl"
import "a_b.kcl"
import "a-b.kcl" as b
import * from "a.kcl"
export import a as aaa from "a.kcl"
export import a, b from "a.kcl"
export import a as aaa, b from "a.kcl"
export import a, b as bbb from "a.kcl"
"#;
        let program = crate::parsing::top_level_parse(input).unwrap();
        let output = program.recast(&Default::default(), 0);
        assert_eq!(output, input);
    }

    #[test]
    fn test_recast_import_as_same_name() {
        let input = r#"import a as a from "a.kcl"
"#;
        let program = crate::parsing::top_level_parse(input).unwrap();
        let output = program.recast(&Default::default(), 0);
        let expected = r#"import a from "a.kcl"
"#;
        assert_eq!(output, expected);
    }

    #[test]
    fn test_recast_export_fn() {
        let input = r#"export fn a() {
  return 0
}
"#;
        let program = crate::parsing::top_level_parse(input).unwrap();
        let output = program.recast(&Default::default(), 0);
        assert_eq!(output, input);
    }

    #[test]
    fn test_recast_bug_fn_in_fn() {
        let some_program_string = r#"// Start point (top left)
zoo_x = -20
zoo_y = 7
// Scale
s = 1 // s = 1 -> height of Z is 13.4mm
// Depth
d = 1

fn rect(x, y, w, h) {
  startSketchOn('XY')
    |> startProfileAt([x, y], %)
    |> xLine(w, %)
    |> yLine(h, %)
    |> xLine(-w, %)
    |> close(%)
    |> extrude(d, %)
}

fn quad(x1, y1, x2, y2, x3, y3, x4, y4) {
  startSketchOn('XY')
    |> startProfileAt([x1, y1], %)
    |> lineTo([x2, y2], %)
    |> lineTo([x3, y3], %)
    |> lineTo([x4, y4], %)
    |> close(%)
    |> extrude(d, %)
}

fn crosshair(x, y) {
  startSketchOn('XY')
    |> startProfileAt([x, y], %)
    |> yLine(1, %)
    |> yLine(-2, %)
    |> yLine(1, %)
    |> xLine(1, %)
    |> xLine(-2, %)
}

fn z(z_x, z_y) {
  z_end_w = s * 8.4
  z_end_h = s * 3
  z_corner = s * 2
  z_w = z_end_w + 2 * z_corner
  z_h = z_w * 1.08130081300813
  rect(z_x, z_y, z_end_w, -z_end_h)
  rect(z_x + z_w, z_y, -z_corner, -z_corner)
  rect(z_x + z_w, z_y - z_h, -z_end_w, z_end_h)
  rect(z_x, z_y - z_h, z_corner, z_corner)
  quad(z_x, z_y - z_h + z_corner, z_x + z_w - z_corner, z_y, z_x + z_w, z_y - z_corner, z_x + z_corner, z_y - z_h)
}

fn o(c_x, c_y) {
  // Outer and inner radii
  o_r = s * 6.95
  i_r = 0.5652173913043478 * o_r

  // Angle offset for diagonal break
  a = 7

  // Start point for the top sketch
  o_x1 = c_x + o_r * cos((45 + a) / 360 * tau())
  o_y1 = c_y + o_r * sin((45 + a) / 360 * tau())

  // Start point for the bottom sketch
  o_x2 = c_x + o_r * cos((225 + a) / 360 * tau())
  o_y2 = c_y + o_r * sin((225 + a) / 360 * tau())

  // End point for the bottom startSketchAt
  o_x3 = c_x + o_r * cos((45 - a) / 360 * tau())
  o_y3 = c_y + o_r * sin((45 - a) / 360 * tau())

  // Where is the center?
  // crosshair(c_x, c_y)


  startSketchOn('XY')
    |> startProfileAt([o_x1, o_y1], %)
    |> arc({
         radius = o_r,
         angle_start = 45 + a,
         angle_end = 225 - a
       }, %)
    |> angledLine([45, o_r - i_r], %)
    |> arc({
         radius = i_r,
         angle_start = 225 - a,
         angle_end = 45 + a
       }, %)
    |> close(%)
    |> extrude(d, %)

  startSketchOn('XY')
    |> startProfileAt([o_x2, o_y2], %)
    |> arc({
         radius = o_r,
         angle_start = 225 + a,
         angle_end = 360 + 45 - a
       }, %)
    |> angledLine([225, o_r - i_r], %)
    |> arc({
         radius = i_r,
         angle_start = 45 - a,
         angle_end = 225 + a - 360
       }, %)
    |> close(%)
    |> extrude(d, %)
}

fn zoo(x0, y0) {
  z(x0, y0)
  o(x0 + s * 20, y0 - (s * 6.7))
  o(x0 + s * 35, y0 - (s * 6.7))
}

zoo(zoo_x, zoo_y)
"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(recasted, some_program_string);
    }

    #[test]
    fn test_recast_bug_extra_parens() {
        let some_program_string = r#"// Ball Bearing
// A ball bearing is a type of rolling-element bearing that uses balls to maintain the separation between the bearing races. The primary purpose of a ball bearing is to reduce rotational friction and support radial and axial loads. 

// Define constants like ball diameter, inside diameter, overhange length, and thickness
sphereDia = 0.5
insideDia = 1
thickness = 0.25
overHangLength = .4

// Sketch and revolve the inside bearing piece
insideRevolve = startSketchOn('XZ')
  |> startProfileAt([insideDia / 2, 0], %)
  |> line([0, thickness + sphereDia / 2], %)
  |> line([overHangLength, 0], %)
  |> line([0, -thickness], %)
  |> line([-overHangLength + thickness, 0], %)
  |> line([0, -sphereDia], %)
  |> line([overHangLength - thickness, 0], %)
  |> line([0, -thickness], %)
  |> line([-overHangLength, 0], %)
  |> close(%)
  |> revolve({ axis: 'y' }, %)

// Sketch and revolve one of the balls and duplicate it using a circular pattern. (This is currently a workaround, we have a bug with rotating on a sketch that touches the rotation axis)
sphere = startSketchOn('XZ')
  |> startProfileAt([
       0.05 + insideDia / 2 + thickness,
       0 - 0.05
     ], %)
  |> line([sphereDia - 0.1, 0], %)
  |> arc({
       angle_start = 0,
       angle_end = -180,
       radius = sphereDia / 2 - 0.05
     }, %)
  |> close(%)
  |> revolve({ axis: 'x' }, %)
  |> patternCircular3d({
       axis = [0, 0, 1],
       center = [0, 0, 0],
       repetitions = 10,
       arcDegrees = 360,
       rotateDuplicates = true
     }, %)

// Sketch and revolve the outside bearing
outsideRevolve = startSketchOn('XZ')
  |> startProfileAt([
       insideDia / 2 + thickness + sphereDia,
       0
     ], %)
  |> line([0, sphereDia / 2], %)
  |> line([-overHangLength + thickness, 0], %)
  |> line([0, thickness], %)
  |> line([overHangLength, 0], %)
  |> line([0, -2 * thickness - sphereDia], %)
  |> line([-overHangLength, 0], %)
  |> line([0, thickness], %)
  |> line([overHangLength - thickness, 0], %)
  |> close(%)
  |> revolve({ axis: 'y' }, %)"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(
            recasted,
            r#"// Ball Bearing
// A ball bearing is a type of rolling-element bearing that uses balls to maintain the separation between the bearing races. The primary purpose of a ball bearing is to reduce rotational friction and support radial and axial loads.


// Define constants like ball diameter, inside diameter, overhange length, and thickness
sphereDia = 0.5
insideDia = 1
thickness = 0.25
overHangLength = .4

// Sketch and revolve the inside bearing piece
insideRevolve = startSketchOn('XZ')
  |> startProfileAt([insideDia / 2, 0], %)
  |> line([0, thickness + sphereDia / 2], %)
  |> line([overHangLength, 0], %)
  |> line([0, -thickness], %)
  |> line([-overHangLength + thickness, 0], %)
  |> line([0, -sphereDia], %)
  |> line([overHangLength - thickness, 0], %)
  |> line([0, -thickness], %)
  |> line([-overHangLength, 0], %)
  |> close(%)
  |> revolve({ axis = 'y' }, %)

// Sketch and revolve one of the balls and duplicate it using a circular pattern. (This is currently a workaround, we have a bug with rotating on a sketch that touches the rotation axis)
sphere = startSketchOn('XZ')
  |> startProfileAt([
       0.05 + insideDia / 2 + thickness,
       0 - 0.05
     ], %)
  |> line([sphereDia - 0.1, 0], %)
  |> arc({
       angle_start = 0,
       angle_end = -180,
       radius = sphereDia / 2 - 0.05
     }, %)
  |> close(%)
  |> revolve({ axis = 'x' }, %)
  |> patternCircular3d({
       axis = [0, 0, 1],
       center = [0, 0, 0],
       repetitions = 10,
       arcDegrees = 360,
       rotateDuplicates = true
     }, %)

// Sketch and revolve the outside bearing
outsideRevolve = startSketchOn('XZ')
  |> startProfileAt([
       insideDia / 2 + thickness + sphereDia,
       0
     ], %)
  |> line([0, sphereDia / 2], %)
  |> line([-overHangLength + thickness, 0], %)
  |> line([0, thickness], %)
  |> line([overHangLength, 0], %)
  |> line([0, -2 * thickness - sphereDia], %)
  |> line([-overHangLength, 0], %)
  |> line([0, thickness], %)
  |> line([overHangLength - thickness, 0], %)
  |> close(%)
  |> revolve({ axis = 'y' }, %)
"#
        );
    }

    #[test]
    fn test_recast_fn_in_object() {
        let some_program_string = r#"bing = { yo = 55 }
myNestedVar = [{ prop = callExp(bing.yo) }]
"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(recasted, some_program_string);
    }

    #[test]
    fn test_recast_fn_in_array() {
        let some_program_string = r#"bing = { yo = 55 }
myNestedVar = [callExp(bing.yo)]
"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(recasted, some_program_string);
    }

    #[test]
    fn test_recast_ranges() {
        let some_program_string = r#"foo = [0..10]
ten = 10
bar = [0 + 1 .. ten]
"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(recasted, some_program_string);
    }

    #[test]
    fn test_recast_space_in_fn_call() {
        let some_program_string = r#"fn thing = (x) => {
    return x + 1
}

thing ( 1 )
"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(
            recasted,
            r#"fn thing(x) {
  return x + 1
}

thing(1)
"#
        );
    }

    #[test]
    fn test_recast_typed_fn() {
        let some_program_string = r#"fn thing(x: string, y: bool[]): number {
  return x + 1
}
"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(recasted, some_program_string);
    }

    #[test]
    fn test_recast_object_fn_in_array_weird_bracket() {
        let some_program_string = r#"bing = { yo = 55 }
myNestedVar = [
  {
  prop:   line([bing.yo, 21], sketch001)
}
]
"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(
            recasted,
            r#"bing = { yo = 55 }
myNestedVar = [
  {
  prop = line([bing.yo, 21], sketch001)
}
]
"#
        );
    }

    #[test]
    fn test_recast_empty_file() {
        let some_program_string = r#""#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        // Its VERY important this comes back with zero new lines.
        assert_eq!(recasted, r#""#);
    }

    #[test]
    fn test_recast_empty_file_new_line() {
        let some_program_string = r#"
"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        // Its VERY important this comes back with zero new lines.
        assert_eq!(recasted, r#""#);
    }

    #[test]
    fn test_recast_shebang() {
        let some_program_string = r#"#!/usr/local/env zoo kcl
part001 = startSketchOn('XY')
  |> startProfileAt([-10, -10], %)
  |> line([20, 0], %)
  |> line([0, 20], %)
  |> line([-20, 0], %)
  |> close(%)
"#;

        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(
            recasted,
            r#"#!/usr/local/env zoo kcl

part001 = startSketchOn('XY')
  |> startProfileAt([-10, -10], %)
  |> line([20, 0], %)
  |> line([0, 20], %)
  |> line([-20, 0], %)
  |> close(%)
"#
        );
    }

    #[test]
    fn test_recast_shebang_new_lines() {
        let some_program_string = r#"#!/usr/local/env zoo kcl
        


part001 = startSketchOn('XY')
  |> startProfileAt([-10, -10], %)
  |> line([20, 0], %)
  |> line([0, 20], %)
  |> line([-20, 0], %)
  |> close(%)
"#;

        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(
            recasted,
            r#"#!/usr/local/env zoo kcl

part001 = startSketchOn('XY')
  |> startProfileAt([-10, -10], %)
  |> line([20, 0], %)
  |> line([0, 20], %)
  |> line([-20, 0], %)
  |> close(%)
"#
        );
    }

    #[test]
    fn test_recast_shebang_with_comments() {
        let some_program_string = r#"#!/usr/local/env zoo kcl
        
// Yo yo my comments.
part001 = startSketchOn('XY')
  |> startProfileAt([-10, -10], %)
  |> line([20, 0], %)
  |> line([0, 20], %)
  |> line([-20, 0], %)
  |> close(%)
"#;

        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(
            recasted,
            r#"#!/usr/local/env zoo kcl

// Yo yo my comments.
part001 = startSketchOn('XY')
  |> startProfileAt([-10, -10], %)
  |> line([20, 0], %)
  |> line([0, 20], %)
  |> line([-20, 0], %)
  |> close(%)
"#
        );
    }

    #[test]
    fn test_recast_large_file() {
        let some_program_string = r#"@settings(units=mm)
// define nts
radius = 6.0
width = 144.0
length = 83.0
depth = 45.0
thk = 5
hole_diam = 5
// define a rectangular shape func
fn rectShape = (pos, w, l) => {
  rr = startSketchOn('xy')
    |> startProfileAt([pos[0] - (w / 2), pos[1] - (l / 2)], %)
    |> lineTo([pos[0] + w / 2, pos[1] - (l / 2)], %,$edge1)
    |> lineTo([pos[0] + w / 2, pos[1] + l / 2], %, $edge2)
    |> lineTo([pos[0] - (w / 2), pos[1] + l / 2], %, $edge3)
    |> close(%, $edge4)
  return rr
}
// build the body of the focusrite scarlett solo gen 4
// only used for visualization
scarlett_body = rectShape([0, 0], width, length)
  |> extrude(depth, %)
  |> fillet({
       radius = radius,
       tags = [
  edge2,
  edge4,
  getOppositeEdge(edge2),
  getOppositeEdge(edge4)
]
     }, %)
  // build the bracket sketch around the body
fn bracketSketch = (w, d, t) => {
  s = startSketchOn({
         plane: {
  origin: { x = 0, y = length / 2 + thk, z = 0 },
  x_axis: { x = 1, y = 0, z = 0 },
  y_axis: { x = 0, y = 0, z = 1 },
  z_axis: { x = 0, y = 1, z = 0 }
}
       })
    |> startProfileAt([-w / 2 - t, d + t], %)
    |> lineTo([-w / 2 - t, -t], %, $edge1)
    |> lineTo([w / 2 + t, -t], %, $edge2)
    |> lineTo([w / 2 + t, d + t], %, $edge3)
    |> lineTo([w / 2, d + t], %, $edge4)
    |> lineTo([w / 2, 0], %, $edge5)
    |> lineTo([-w / 2, 0], %, $edge6)
    |> lineTo([-w / 2, d + t], %, $edge7)
    |> close(%, $edge8)
  return s
}
// build the body of the bracket
bracket_body = bracketSketch(width, depth, thk)
  |> extrude(length + 10, %)
  |> fillet({
       radius = radius,
       tags: [
  getNextAdjacentEdge(edge7),
  getNextAdjacentEdge(edge2),
  getNextAdjacentEdge(edge3),
  getNextAdjacentEdge(edge6)
]
     }, %)
  // build the tabs of the mounting bracket (right side)
tabs_r = startSketchOn({
       plane: {
  origin: { x = 0, y = 0, z = depth + thk },
  x_axis: { x = 1, y = 0, z = 0 },
  y_axis: { x = 0, y = 1, z = 0 },
  z_axis: { x = 0, y = 0, z = 1 }
}
     })
  |> startProfileAt([width / 2 + thk, length / 2 + thk], %)
  |> line([10, -5], %)
  |> line([0, -10], %)
  |> line([-10, -5], %)
  |> close(%)
  |> hole(circle({
       center = [
         width / 2 + thk + hole_diam,
         length / 2 - hole_diam
       ],
       radius = hole_diam / 2
     }, %), %)
  |> extrude(-thk, %)
  |> patternLinear3d({
       axis = [0, -1, 0],
       repetitions = 1,
       distance = length - 10
     }, %)
  // build the tabs of the mounting bracket (left side)
tabs_l = startSketchOn({
       plane: {
  origin = { x = 0, y = 0, z = depth + thk },
  x_axis = { x = 1, y = 0, z = 0 },
  y_axis = { x = 0, y = 1, z = 0 },
  z_axis = { x = 0, y = 0, z = 1 }
}
     })
  |> startProfileAt([-width / 2 - thk, length / 2 + thk], %)
  |> line([-10, -5], %)
  |> line([0, -10], %)
  |> line([10, -5], %)
  |> close(%)
  |> hole(circle({
       center = [
         -width / 2 - thk - hole_diam,
         length / 2 - hole_diam
       ],
       radius = hole_diam / 2
     }, %), %)
  |> extrude(-thk, %)
  |> patternLinear3d({
       axis = [0, -1, 0],
       repetitions = 1,
       distance = length - 10
     }, %)
"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        // Its VERY important this comes back with zero new lines.
        assert_eq!(
            recasted,
            r#"@settings(units = mm)
// define nts
radius = 6.0
width = 144.0
length = 83.0
depth = 45.0
thk = 5
hole_diam = 5
// define a rectangular shape func
fn rectShape(pos, w, l) {
  rr = startSketchOn('xy')
    |> startProfileAt([pos[0] - (w / 2), pos[1] - (l / 2)], %)
    |> lineTo([pos[0] + w / 2, pos[1] - (l / 2)], %, $edge1)
    |> lineTo([pos[0] + w / 2, pos[1] + l / 2], %, $edge2)
    |> lineTo([pos[0] - (w / 2), pos[1] + l / 2], %, $edge3)
    |> close(%, $edge4)
  return rr
}
// build the body of the focusrite scarlett solo gen 4
// only used for visualization
scarlett_body = rectShape([0, 0], width, length)
  |> extrude(depth, %)
  |> fillet({
       radius = radius,
       tags = [
         edge2,
         edge4,
         getOppositeEdge(edge2),
         getOppositeEdge(edge4)
       ]
     }, %)
// build the bracket sketch around the body
fn bracketSketch(w, d, t) {
  s = startSketchOn({
         plane = {
           origin = { x = 0, y = length / 2 + thk, z = 0 },
           x_axis = { x = 1, y = 0, z = 0 },
           y_axis = { x = 0, y = 0, z = 1 },
           z_axis = { x = 0, y = 1, z = 0 }
         }
       })
    |> startProfileAt([-w / 2 - t, d + t], %)
    |> lineTo([-w / 2 - t, -t], %, $edge1)
    |> lineTo([w / 2 + t, -t], %, $edge2)
    |> lineTo([w / 2 + t, d + t], %, $edge3)
    |> lineTo([w / 2, d + t], %, $edge4)
    |> lineTo([w / 2, 0], %, $edge5)
    |> lineTo([-w / 2, 0], %, $edge6)
    |> lineTo([-w / 2, d + t], %, $edge7)
    |> close(%, $edge8)
  return s
}
// build the body of the bracket
bracket_body = bracketSketch(width, depth, thk)
  |> extrude(length + 10, %)
  |> fillet({
       radius = radius,
       tags = [
         getNextAdjacentEdge(edge7),
         getNextAdjacentEdge(edge2),
         getNextAdjacentEdge(edge3),
         getNextAdjacentEdge(edge6)
       ]
     }, %)
// build the tabs of the mounting bracket (right side)
tabs_r = startSketchOn({
       plane = {
         origin = { x = 0, y = 0, z = depth + thk },
         x_axis = { x = 1, y = 0, z = 0 },
         y_axis = { x = 0, y = 1, z = 0 },
         z_axis = { x = 0, y = 0, z = 1 }
       }
     })
  |> startProfileAt([width / 2 + thk, length / 2 + thk], %)
  |> line([10, -5], %)
  |> line([0, -10], %)
  |> line([-10, -5], %)
  |> close(%)
  |> hole(circle({
       center = [
         width / 2 + thk + hole_diam,
         length / 2 - hole_diam
       ],
       radius = hole_diam / 2
     }, %), %)
  |> extrude(-thk, %)
  |> patternLinear3d({
       axis = [0, -1, 0],
       repetitions = 1,
       distance = length - 10
     }, %)
// build the tabs of the mounting bracket (left side)
tabs_l = startSketchOn({
       plane = {
         origin = { x = 0, y = 0, z = depth + thk },
         x_axis = { x = 1, y = 0, z = 0 },
         y_axis = { x = 0, y = 1, z = 0 },
         z_axis = { x = 0, y = 0, z = 1 }
       }
     })
  |> startProfileAt([-width / 2 - thk, length / 2 + thk], %)
  |> line([-10, -5], %)
  |> line([0, -10], %)
  |> line([10, -5], %)
  |> close(%)
  |> hole(circle({
       center = [
         -width / 2 - thk - hole_diam,
         length / 2 - hole_diam
       ],
       radius = hole_diam / 2
     }, %), %)
  |> extrude(-thk, %)
  |> patternLinear3d({
       axis = [0, -1, 0],
       repetitions = 1,
       distance = length - 10
     }, %)
"#
        );
    }

    #[test]
    fn test_recast_nested_var_declaration_in_fn_body() {
        let some_program_string = r#"fn cube = (pos, scale) => {
   sg = startSketchOn('XY')
  |> startProfileAt(pos, %)
  |> line([0, scale], %)
  |> line([scale, 0], %)
  |> line([0, -scale], %)
  |> close(%)
  |> extrude(scale, %)
}"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(
            recasted,
            r#"fn cube(pos, scale) {
  sg = startSketchOn('XY')
    |> startProfileAt(pos, %)
    |> line([0, scale], %)
    |> line([scale, 0], %)
    |> line([0, -scale], %)
    |> close(%)
    |> extrude(scale, %)
}
"#
        );
    }

    #[test]
    fn test_as() {
        let some_program_string = r#"fn cube(pos, scale) {
  x = dfsfs + dfsfsd as y

  sg = startSketchOn('XY')
    |> startProfileAt(pos, %) as foo
    |> line([0, scale], %)
    |> line([scale, 0], %) as bar
    |> line([0 as baz, -scale] as qux, %)
    |> close(%)
    |> extrude(scale, %)
}

cube(0, 0) as cub
"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(recasted, some_program_string,);
    }

    #[test]
    fn test_recast_with_bad_indentation() {
        let some_program_string = r#"part001 = startSketchOn('XY')
  |> startProfileAt([0.0, 5.0], %)
              |> line([0.4900857016, -0.0240763666], %)
    |> line([0.6804562304, 0.9087880491], %)"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(
            recasted,
            r#"part001 = startSketchOn('XY')
  |> startProfileAt([0.0, 5.0], %)
  |> line([0.4900857016, -0.0240763666], %)
  |> line([0.6804562304, 0.9087880491], %)
"#
        );
    }

    #[test]
    fn test_recast_with_bad_indentation_and_inline_comment() {
        let some_program_string = r#"part001 = startSketchOn('XY')
  |> startProfileAt([0.0, 5.0], %)
              |> line([0.4900857016, -0.0240763666], %) // hello world
    |> line([0.6804562304, 0.9087880491], %)"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(
            recasted,
            r#"part001 = startSketchOn('XY')
  |> startProfileAt([0.0, 5.0], %)
  |> line([0.4900857016, -0.0240763666], %) // hello world
  |> line([0.6804562304, 0.9087880491], %)
"#
        );
    }
    #[test]
    fn test_recast_with_bad_indentation_and_line_comment() {
        let some_program_string = r#"part001 = startSketchOn('XY')
  |> startProfileAt([0.0, 5.0], %)
              |> line([0.4900857016, -0.0240763666], %)
        // hello world
    |> line([0.6804562304, 0.9087880491], %)"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(
            recasted,
            r#"part001 = startSketchOn('XY')
  |> startProfileAt([0.0, 5.0], %)
  |> line([0.4900857016, -0.0240763666], %)
  // hello world
  |> line([0.6804562304, 0.9087880491], %)
"#
        );
    }

    #[test]
    fn test_recast_comment_in_a_fn_block() {
        let some_program_string = r#"fn myFn = () => {
  // this is a comment
  yo = { a = { b = { c = '123' } } } /* block
  comment */

  key = 'c'
  // this is also a comment
    return things
}"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(
            recasted,
            r#"fn myFn() {
  // this is a comment
  yo = { a = { b = { c = '123' } } } /* block
  comment */

  key = 'c'
  // this is also a comment
  return things
}
"#
        );
    }

    #[test]
    fn test_recast_comment_under_variable() {
        let some_program_string = r#"key = 'c'
// this is also a comment
thing = 'foo'
"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(
            recasted,
            r#"key = 'c'
// this is also a comment
thing = 'foo'
"#
        );
    }

    #[test]
    fn test_recast_multiline_comment_start_file() {
        let some_program_string = r#"// hello world
// I am a comment
key = 'c'
// this is also a comment
// hello
thing = 'foo'
"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(
            recasted,
            r#"// hello world
// I am a comment
key = 'c'
// this is also a comment
// hello
thing = 'foo'
"#
        );
    }

    #[test]
    fn test_recast_empty_comment() {
        let some_program_string = r#"// hello world
//
// I am a comment
key = 'c'

//
// I am a comment
thing = 'c'

foo = 'bar' //
"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(
            recasted,
            r#"// hello world
//
// I am a comment
key = 'c'

//
// I am a comment
thing = 'c'

foo = 'bar' //
"#
        );
    }

    #[test]
    fn test_recast_multiline_comment_under_variable() {
        let some_program_string = r#"key = 'c'
// this is also a comment
// hello
thing = 'foo'
"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(
            recasted,
            r#"key = 'c'
// this is also a comment
// hello
thing = 'foo'
"#
        );
    }

    #[test]
    fn test_recast_comment_at_start() {
        let test_program = r#"
/* comment at start */

mySk1 = startSketchAt([0, 0])"#;
        let program = crate::parsing::top_level_parse(test_program).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(
            recasted,
            r#"/* comment at start */

mySk1 = startSketchAt([0, 0])
"#
        );
    }

    #[test]
    fn test_recast_lots_of_comments() {
        let some_program_string = r#"// comment at start
mySk1 = startSketchOn('XY')
  |> startProfileAt([0, 0], %)
  |> lineTo([1, 1], %)
  // comment here
  |> lineTo([0, 1], %, $myTag)
  |> lineTo([1, 1], %)
  /* and
  here
  */
  // a comment between pipe expression statements
  |> rx(90, %)
  // and another with just white space between others below
  |> ry(45, %)
  |> rx(45, %)
// one more for good measure"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(
            recasted,
            r#"// comment at start
mySk1 = startSketchOn('XY')
  |> startProfileAt([0, 0], %)
  |> lineTo([1, 1], %)
  // comment here
  |> lineTo([0, 1], %, $myTag)
  |> lineTo([1, 1], %)
  /* and
  here */
  // a comment between pipe expression statements
  |> rx(90, %)
  // and another with just white space between others below
  |> ry(45, %)
  |> rx(45, %)
// one more for good measure
"#
        );
    }

    #[test]
    fn test_recast_multiline_object() {
        let some_program_string = r#"part001 = startSketchOn('XY')
  |> startProfileAt([-0.01, -0.08], %)
  |> line([0.62, 4.15], %, $seg01)
  |> line([2.77, -1.24], %)
  |> angledLineThatIntersects({
       angle = 201,
       offset = -1.35,
       intersectTag = seg01
     }, %)
  |> line([-0.42, -1.72], %)"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(recasted.trim(), some_program_string);
    }

    #[test]
    fn test_recast_first_level_object() {
        let some_program_string = r#"three = 3

yo = {
  aStr = 'str',
  anum = 2,
  identifier = three,
  binExp = 4 + 5
}
yo = [
  1,
  "  2,",
  "three",
  4 + 5,
  "  hey oooooo really long long long"
]
"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(recasted, some_program_string);
    }

    #[test]
    fn test_recast_new_line_before_comment() {
        let some_program_string = r#"
// this is a comment
yo = { a = { b = { c = '123' } } }

key = 'c'
things = "things"

// this is also a comment"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        let expected = some_program_string.trim();
        // Currently new parser removes an empty line
        let actual = recasted.trim();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_recast_comment_tokens_inside_strings() {
        let some_program_string = r#"b = {
  end = 141,
  start = 125,
  type_ = "NonCodeNode",
  value = "
 // a comment
   "
}"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(recasted.trim(), some_program_string.trim());
    }

    #[test]
    fn test_recast_array_new_line_in_pipe() {
        let some_program_string = r#"myVar = 3
myVar2 = 5
myVar3 = 6
myAng = 40
myAng2 = 134
part001 = startSketchOn('XY')
  |> startProfileAt([0, 0], %)
  |> line([1, 3.82], %, $seg01) // ln-should-get-tag
  |> angledLineToX([
       -angleToMatchLengthX(seg01, myVar, %),
       myVar
     ], %) // ln-lineTo-xAbsolute should use angleToMatchLengthX helper
  |> angledLineToY([
       -angleToMatchLengthY(seg01, myVar, %),
       myVar
     ], %) // ln-lineTo-yAbsolute should use angleToMatchLengthY helper"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(recasted.trim(), some_program_string);
    }

    #[test]
    fn test_recast_array_new_line_in_pipe_custom() {
        let some_program_string = r#"myVar = 3
myVar2 = 5
myVar3 = 6
myAng = 40
myAng2 = 134
part001 = startSketchOn('XY')
   |> startProfileAt([0, 0], %)
   |> line([1, 3.82], %, $seg01) // ln-should-get-tag
   |> angledLineToX([
         -angleToMatchLengthX(seg01, myVar, %),
         myVar
      ], %) // ln-lineTo-xAbsolute should use angleToMatchLengthX helper
   |> angledLineToY([
         -angleToMatchLengthY(seg01, myVar, %),
         myVar
      ], %) // ln-lineTo-yAbsolute should use angleToMatchLengthY helper
"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(
            &FormatOptions {
                tab_size: 3,
                use_tabs: false,
                insert_final_newline: true,
            },
            0,
        );
        assert_eq!(recasted, some_program_string);
    }

    #[test]
    fn test_recast_after_rename_std() {
        let some_program_string = r#"part001 = startSketchOn('XY')
  |> startProfileAt([0.0000000000, 5.0000000000], %)
    |> line([0.4900857016, -0.0240763666], %)

part002 = "part002"
things = [part001, 0.0]
blah = 1
foo = false
baz = {a: 1, part001: "thing"}

fn ghi = (part001) => {
  return part001
}
"#;
        let mut program = crate::parsing::top_level_parse(some_program_string).unwrap();
        program.rename_symbol("mySuperCoolPart", 6);

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(
            recasted,
            r#"mySuperCoolPart = startSketchOn('XY')
  |> startProfileAt([0.0, 5.0], %)
  |> line([0.4900857016, -0.0240763666], %)

part002 = "part002"
things = [mySuperCoolPart, 0.0]
blah = 1
foo = false
baz = { a = 1, part001 = "thing" }

fn ghi(part001) {
  return part001
}
"#
        );
    }

    #[test]
    fn test_recast_after_rename_fn_args() {
        let some_program_string = r#"fn ghi = (x, y, z) => {
  return x
}"#;
        let mut program = crate::parsing::top_level_parse(some_program_string).unwrap();
        program.rename_symbol("newName", 10);

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(
            recasted,
            r#"fn ghi(newName, y, z) {
  return newName
}
"#
        );
    }

    #[test]
    fn test_recast_trailing_comma() {
        let some_program_string = r#"startSketchOn('XY')
  |> startProfileAt([0, 0], %)
  |> arc({
    radius = 1,
    angle_start = 0,
    angle_end = 180,
  }, %)"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(
            recasted,
            r#"startSketchOn('XY')
  |> startProfileAt([0, 0], %)
  |> arc({
       radius = 1,
       angle_start = 0,
       angle_end = 180
     }, %)
"#
        );
    }

    #[test]
    fn test_recast_negative_var() {
        let some_program_string = r#"w = 20
l = 8
h = 10

firstExtrude = startSketchOn('XY')
  |> startProfileAt([0,0], %)
  |> line([0, l], %)
  |> line([w, 0], %)
  |> line([0, -l], %)
  |> close(%)
  |> extrude(h, %)
"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(
            recasted,
            r#"w = 20
l = 8
h = 10

firstExtrude = startSketchOn('XY')
  |> startProfileAt([0, 0], %)
  |> line([0, l], %)
  |> line([w, 0], %)
  |> line([0, -l], %)
  |> close(%)
  |> extrude(h, %)
"#
        );
    }

    #[test]
    fn test_recast_multiline_comment() {
        let some_program_string = r#"w = 20
l = 8
h = 10

// This is my comment
// It has multiple lines
// And it's really long
firstExtrude = startSketchOn('XY')
  |> startProfileAt([0,0], %)
  |> line([0, l], %)
  |> line([w, 0], %)
  |> line([0, -l], %)
  |> close(%)
  |> extrude(h, %)
"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(
            recasted,
            r#"w = 20
l = 8
h = 10

// This is my comment
// It has multiple lines
// And it's really long
firstExtrude = startSketchOn('XY')
  |> startProfileAt([0, 0], %)
  |> line([0, l], %)
  |> line([w, 0], %)
  |> line([0, -l], %)
  |> close(%)
  |> extrude(h, %)
"#
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_recast_math_start_negative() {
        let some_program_string = r#"myVar = -5 + 6"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(recasted.trim(), some_program_string);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_recast_math_negate_parens() {
        let some_program_string = r#"wallMountL = 3.82
thickness = 0.5

startSketchOn('XY')
  |> startProfileAt([0, 0], %)
  |> line([0, -(wallMountL - thickness)], %)
  |> line([0, -(5 - thickness)], %)
  |> line([0, -(5 - 1)], %)
  |> line([0, -(-5 - 1)], %)"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(recasted.trim(), some_program_string);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_recast_math_nested_parens() {
        let some_program_string = r#"distance = 5
p = 3
FOS = 2
sigmaAllow = 8
width = 20
thickness = sqrt(distance * p * FOS * 6 / (sigmaAllow * width))"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(recasted.trim(), some_program_string);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn no_vardec_keyword() {
        let some_program_string = r#"distance = 5"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();

        let recasted = program.recast(&Default::default(), 0);
        assert_eq!(recasted.trim(), some_program_string);
    }

    #[test]
    fn recast_nested_fn() {
        let some_program_string = r#"fn f = () => {
  return fn() => {
  return 1
}
}"#;
        let program = crate::parsing::top_level_parse(some_program_string).unwrap();
        let recasted = program.recast(&Default::default(), 0);
        let expected = "\
fn f() {
  return fn() {
    return 1
  }
}";
        assert_eq!(recasted.trim(), expected);
    }

    #[test]
    fn recast_literal() {
        use winnow::Parser;
        for (i, (raw, expected, reason)) in [
            (
                "5.0",
                "5.0",
                "fractional numbers should stay fractional, i.e. don't reformat this to '5'",
            ),
            (
                "5",
                "5",
                "integers should stay integral, i.e. don't reformat this to '5.0'",
            ),
            (
                "5.0000000",
                "5.0",
                "if the number is f64 but not fractional, use its canonical format",
            ),
            ("5.1", "5.1", "straightforward case works"),
        ]
        .into_iter()
        .enumerate()
        {
            let tokens = crate::parsing::token::lex(raw, ModuleId::default()).unwrap();
            let literal = crate::parsing::parser::unsigned_number_literal
                .parse(tokens.as_slice())
                .unwrap();
            assert_eq!(
                literal.recast(),
                expected,
                "failed test {i}, which is testing that {reason}"
            );
        }
    }

    #[test]
    fn recast_objects_no_comments() {
        let input = r#"
sketch002 = startSketchOn({
       plane: {
    origin: { x = 1, y = 2, z = 3 },
    x_axis: { x = 4, y = 5, z = 6 },
    y_axis: { x = 7, y = 8, z = 9 },
    z_axis: { x = 10, y = 11, z = 12 }
       }
  })
"#;
        let expected = r#"sketch002 = startSketchOn({
  plane = {
    origin = { x = 1, y = 2, z = 3 },
    x_axis = { x = 4, y = 5, z = 6 },
    y_axis = { x = 7, y = 8, z = 9 },
    z_axis = { x = 10, y = 11, z = 12 }
  }
})
"#;
        let ast = crate::parsing::top_level_parse(input).unwrap();
        let actual = ast.recast(&FormatOptions::new(), 0);
        assert_eq!(actual, expected);
    }

    #[test]
    fn unparse_fn_unnamed() {
        let input = r#"squares_out = reduce(arr, 0, fn(i, squares) {
  return 1
})
"#;
        let ast = crate::parsing::top_level_parse(input).unwrap();
        let actual = ast.recast(&FormatOptions::new(), 0);
        assert_eq!(actual, input);
    }

    #[test]
    fn unparse_fn_named() {
        let input = r#"fn f(x) {
  return 1
}
"#;
        let ast = crate::parsing::top_level_parse(input).unwrap();
        let actual = ast.recast(&FormatOptions::new(), 0);
        assert_eq!(actual, input);
    }

    #[test]
    fn recast_objects_with_comments() {
        use winnow::Parser;
        for (i, (input, expected, reason)) in [(
            "\
{
  a = 1,
  // b = 2,
  c = 3
}",
            "\
{
  a = 1,
  // b = 2,
  c = 3
}",
            "preserves comments",
        )]
        .into_iter()
        .enumerate()
        {
            let tokens = crate::parsing::token::lex(input, ModuleId::default()).unwrap();
            crate::parsing::parser::print_tokens(tokens.as_slice());
            let expr = crate::parsing::parser::object.parse(tokens.as_slice()).unwrap();
            assert_eq!(
                expr.recast(&FormatOptions::new(), 0, ExprContext::Other),
                expected,
                "failed test {i}, which is testing that recasting {reason}"
            );
        }
    }

    #[test]
    fn recast_array_with_comments() {
        use winnow::Parser;
        for (i, (input, expected, reason)) in [
            (
                "\
[
  1,
  2,
  3,
  4,
  5,
  6,
  7,
  8,
  9,
  10,
  11,
  12,
  13,
  14,
  15,
  16,
  17,
  18,
  19,
  20,
]",
                "\
[
  1,
  2,
  3,
  4,
  5,
  6,
  7,
  8,
  9,
  10,
  11,
  12,
  13,
  14,
  15,
  16,
  17,
  18,
  19,
  20
]",
                "preserves multi-line arrays",
            ),
            (
                "\
[
  1,
  // 2,
  3
]",
                "\
[
  1,
  // 2,
  3
]",
                "preserves comments",
            ),
            (
                "\
[
  1,
  2,
  // 3
]",
                "\
[
  1,
  2,
  // 3
]",
                "preserves comments at the end of the array",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let tokens = crate::parsing::token::lex(input, ModuleId::default()).unwrap();
            let expr = crate::parsing::parser::array_elem_by_elem
                .parse(tokens.as_slice())
                .unwrap();
            assert_eq!(
                expr.recast(&FormatOptions::new(), 0, ExprContext::Other),
                expected,
                "failed test {i}, which is testing that recasting {reason}"
            );
        }
    }
}

use ratatui::style::Color;
use std::sync::OnceLock;
use syntect::parsing::Scope;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SemanticRole {
    Fg,
    Comment,
    String,
    Constant,
    Keyword,
    Function,
    Type,
    Parameter,
    Tag,
    Operator,
    Macro,
    Invalid,
}

struct ScopeSelectors {
    comment: [Scope; 1],
    string: [Scope; 1],
    constant: [Scope; 2],
    keyword: [Scope; 2],
    function: [Scope; 3],
    type_name: [Scope; 4],
    parameter: [Scope; 3],
    shell_variable: [Scope; 4],
    tag: [Scope; 3],
    operator: [Scope; 3],
    macro_name: [Scope; 4],
    invalid: [Scope; 2],
    variable_readwrite: [Scope; 1],
    shell_source: [Scope; 1],
    shell_function_call: [Scope; 1],
    shell_function_arguments: [Scope; 1],
}

pub(super) fn semantic_role_for_token(text: &str, scope_stack: &[Scope]) -> SemanticRole {
    let selectors = scope_selectors();

    if scope_stack_matches(scope_stack, &selectors.invalid) {
        SemanticRole::Invalid
    } else if scope_stack_matches(scope_stack, &selectors.comment) {
        SemanticRole::Comment
    } else if scope_stack_matches(scope_stack, &selectors.string) {
        SemanticRole::String
    } else if scope_stack_matches(scope_stack, &selectors.macro_name) {
        SemanticRole::Macro
    } else if scope_stack_matches(scope_stack, &selectors.shell_variable)
        || scope_stack_matches(scope_stack, &selectors.parameter)
    {
        SemanticRole::Parameter
    } else if scope_stack_matches(scope_stack, &selectors.tag) {
        SemanticRole::Tag
    } else if scope_stack_matches(scope_stack, &selectors.function) {
        SemanticRole::Function
    } else if scope_stack_matches(scope_stack, &selectors.type_name)
        || (scope_stack_matches(scope_stack, &selectors.variable_readwrite)
            && text.chars().next().is_some_and(char::is_uppercase))
    {
        SemanticRole::Type
    } else if scope_stack_matches(scope_stack, &selectors.operator) {
        SemanticRole::Operator
    } else if scope_stack_matches(scope_stack, &selectors.keyword) {
        SemanticRole::Keyword
    } else if scope_stack_matches(scope_stack, &selectors.constant) {
        SemanticRole::Constant
    } else if let Some(role) = shell_semantic_role_from_heuristics(text, scope_stack, selectors) {
        role
    } else {
        SemanticRole::Fg
    }
}

pub(super) fn looks_like_shell_command_name(token: &str) -> bool {
    let mut chars = token.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
}

pub(super) fn role_color(
    role: SemanticRole,
    palette: crate::ui::theme::CodePreviewPalette,
) -> Color {
    match role {
        SemanticRole::Fg => palette.fg,
        SemanticRole::Comment => palette.comment,
        SemanticRole::String => palette.string,
        SemanticRole::Constant => palette.constant,
        SemanticRole::Keyword => palette.keyword,
        SemanticRole::Function => palette.function,
        SemanticRole::Type => palette.r#type,
        SemanticRole::Parameter => palette.parameter,
        SemanticRole::Tag => palette.tag,
        SemanticRole::Operator => palette.operator,
        SemanticRole::Macro => palette.r#macro,
        SemanticRole::Invalid => palette.invalid,
    }
}

fn scope_selectors() -> &'static ScopeSelectors {
    static SELECTORS: OnceLock<ScopeSelectors> = OnceLock::new();
    SELECTORS.get_or_init(|| {
        let scope = |value| Scope::new(value).expect("valid syntect scope selector");
        ScopeSelectors {
            comment: [scope("comment")],
            string: [scope("string")],
            constant: [scope("constant"), scope("support.constant")],
            keyword: [scope("keyword"), scope("storage")],
            function: [
                scope("entity.name.function"),
                scope("support.function"),
                scope("variable.function"),
            ],
            type_name: [
                scope("entity.name.type"),
                scope("entity.name.class"),
                scope("support.type"),
                scope("support.class"),
            ],
            parameter: [
                scope("variable.parameter"),
                scope("entity.other.attribute-name"),
                scope("variable.other.readwrite.assignment"),
            ],
            shell_variable: [
                scope("meta.group.expansion.parameter"),
                scope("punctuation.definition.variable"),
                scope("variable.other.readwrite.shell"),
                scope("variable.language.shell"),
            ],
            tag: [
                scope("entity.name.tag"),
                scope("meta.tag"),
                scope("punctuation.definition.tag"),
            ],
            operator: [
                scope("keyword.operator"),
                scope("punctuation.separator.key-value"),
                scope("punctuation.accessor"),
            ],
            macro_name: [
                scope("entity.name.function.preprocessor"),
                scope("support.function.preprocessor"),
                scope("meta.preprocessor"),
                scope("keyword.directive"),
            ],
            invalid: [scope("invalid"), scope("invalid.deprecated")],
            variable_readwrite: [scope("variable.other.readwrite")],
            shell_source: [scope("source.shell")],
            shell_function_call: [scope("meta.function-call")],
            shell_function_arguments: [scope("meta.function-call.arguments")],
        }
    })
}

fn shell_semantic_role_from_heuristics(
    text: &str,
    scope_stack: &[Scope],
    selectors: &ScopeSelectors,
) -> Option<SemanticRole> {
    if !scope_stack_matches(scope_stack, &selectors.shell_source) {
        return None;
    }

    let token = text.trim();
    if token.is_empty() {
        return None;
    }

    if matches!(
        token,
        "if" | "then"
            | "fi"
            | "for"
            | "do"
            | "done"
            | "case"
            | "esac"
            | "while"
            | "until"
            | "in"
            | "elif"
            | "else"
            | "select"
    ) {
        return Some(SemanticRole::Keyword);
    }

    if matches!(token, "[" | "]" | "test" | "echo" | "printf") {
        return Some(SemanticRole::Function);
    }

    if scope_stack_matches(scope_stack, &selectors.shell_function_call)
        && looks_like_shell_command_name(token)
    {
        return Some(SemanticRole::Function);
    }

    if scope_stack_matches(scope_stack, &selectors.shell_function_arguments)
        && token.starts_with('-')
    {
        return Some(SemanticRole::Parameter);
    }

    if token.starts_with('$') || token.starts_with("${") || token.starts_with("$(") {
        return Some(SemanticRole::Parameter);
    }

    if looks_like_shell_assignment_name(token) {
        return Some(SemanticRole::Parameter);
    }

    None
}

fn looks_like_shell_assignment_name(token: &str) -> bool {
    let mut chars = token.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_uppercase() || first == '_') {
        return false;
    }
    chars.all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
}

fn scope_stack_matches(scope_stack: &[Scope], selectors: &[Scope]) -> bool {
    scope_stack.iter().rev().any(|scope| {
        selectors
            .iter()
            .any(|selector| selector.is_prefix_of(*scope))
    })
}

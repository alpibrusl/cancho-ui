# Grammar sketch

> **Status: Phase 0 sketch, not implemented.** Phase 1 produces a
> parser; this document is the EBNF the parser meets. A rule that
> lands in the parser locks the rule in the language, the same way
> a refusal test in `examples/refusal/` locks a refusal rule.

---

## 1. Position

The grammar is the smallest EBNF that produces the syntax in
[`docs/language-design.md`](../../docs/language-design.md) §3.
It is deliberately hand-written rather than parser-generator
output; Phase 1 may switch to a generator (Lark, pest, lalrpop)
once the shape is settled. The grammar is not the language
design — it is the contract the design makes with the parser.

The grammar does **not** specify:

- Type rules (those live in [`docs/semantics.md`](../../docs/semantics.md)
  and the type checker's own implementation).
- Effect / capability rules (same).
- Static analyses (same).
- The view tree's runtime semantics (Phase 2+).

---

## 2. Reserved words

The reserved words are exactly the keywords in
[`docs/language-design.md`](../../docs/language-design.md) §4:

```
component  state  view  on  requires  effect  capability  foreign
module  import  pub  target  let  fn  if  else  match  return
```

Using any of these as a binding name (state field, event,
capability, helper) is refused with `name-reserved`. The
reserved words are listed in
[`spec/diagnostics/README.md`](../diagnostics/README.md) §3 as
a single rule.

---

## 3. Top-level

```ebnf
file        := { declaration } ;
declaration := module_decl
             | import_decl
             | component_decl
             | adapter_decl
             | foreign_decl
             ;
```

### 3.1 `module_decl`

```ebnf
module_decl := "module" identifier ";" ;
```

One per file in Phase 0. The module name becomes the cancho-ui
namespace the file is imported under.

### 3.2 `import_decl`

> **Corrected by Tier 16 (2026-10-18).** The Phase 0 sketch had
> whole-module imports with an `as` alias. What landed in the
> parser is a *component import* — one component of one sibling
> module — because the composition model is per-component
> instances (Tier 12), and that keeps 1:1 the emitted module a
> single file with one auto-mount. The `as` alias and whole-module
> imports return in Phase 6 (packages).

```ebnf
import_decl := "import" identifier "." identifier ";" ;
```

`import cards.HelloCard;` imports exactly one component from the
sibling file `cards.cui` (beside the importing file), which must
declare `module cards;`. The imported component instantiates like
a same-file one. Component names must be unique across the whole
import graph (`component-name-collision`) because the merged
module emits one `mount_<Name>` per component.

### 3.3 `component_decl`

```ebnf
component_decl :=
    "component" identifier "{"
        { component_member }
    "}" ;

component_member :=
      property_decl
    | state_decl
    | requires_decl
    | on_clause
    | view_clause
    | schedule_decl
    | lifecycle_hook
    | target_override
    ;
```

### 3.4 `adapter_decl`

```ebnf
adapter_decl :=
    "adapter" identifier "{"
        { adapter_member }
    "}" ;

adapter_member :=
      requires_decl
    | foreign_decl
    | fn_decl
    ;
```

### 3.5 `foreign_decl`

```ebnf
foreign_decl :=
    "foreign" "{"
        { foreign_fn }
    "}" ;

foreign_fn :=
    "fn" identifier "(" [ arg_list ] ")"
        "->" type
        "from" string_literal
        [ "async" ]
    ";" ;
```

---

## 4. Component members

### 4.1 `property_decl`

```ebnf
property_decl :=
    "prop" identifier ":" type [ "=" expr ] ";" ;
```

### 4.2 `state_decl`

```ebnf
state_decl :=
    "state" identifier ":" type "=" expr ";" ;
```

### 4.3 `requires_decl`

```ebnf
requires_decl :=
    "requires" identifier [ "(" narrowing ")" ] ";" ;

narrowing := string_literal
           | identifier
           | { "," (string_literal | identifier) } ;
```

`narrowing` is Phase 2+; Phase 1 accepts only the bare
identifier form.

### 4.4 `on_clause`

```ebnf
on_clause :=
    "on" identifier [ "(" arg_list ")" ] "{"
        stmt_block
    "}" ;

arg_list :=    ;
arg_list := expr { "," expr } ;
```

### 4.5 `view_clause`

```ebnf
view_clause :=
    "view" "{"
        view_node
    "}" ;
```

### 4.5a `schedule_decl`

```ebnf
schedule_decl :=
      "start" identifier ";"
    | "every" integer_literal identifier ";" ;
```

`start` and `every` are contextual: identifiers everywhere except at the start of a component member. The event
must exist and take no parameters; the period is 100 to 86,400,000 ms. Semantics: [`docs/schedules.md`](../../docs/schedules.md).

### 4.6 `lifecycle_hook`

```ebnf
lifecycle_hook :=
    ( "on_mount" | "on_unmount" | "on_pause" | "on_resume" ) "{"
        stmt_block
    "}" ;
```

### 4.7 `target_override`

```ebnf
target_override :=
    "target" identifier "{"
        view_clause_or_stmt_block
    "}" ;
```

---

## 5. The view tree

```ebnf
view_node :=
      element_node
    | text_node
    | conditional_node
    | each_node
    | slot_node
    ;

element_node :=
    identifier "{" "}"
              | identifier "{" view_node { view_node } "}"
              | identifier "{" accessibility_block view_node { view_node } "}"
              ;

text_node :=
    string_literal
  | string_literal "{" interpolation { "," interpolation } "}"
  ;

interpolation := expr ;

conditional_node :=
    "if" expr "{" view_node "}" [ "else" "{" view_node "}" ] ;

each_node :=
    "each" identifier "in" expr "{" view_node "}" ;

slot_node :=
    "slot" identifier [ "{" view_node "}" ] ;

accessibility_block :=
    "role" ":" identifier
  | "label" ":" text_expr
  | "keyboard" ":" keyboard_action
  ;

keyboard_action :=
    "Enter" "->" identifier
  | "{" { string_literal "->" identifier } "}" ;
```

---

## 6. Statements

```ebnf
stmt_block := { stmt } ;

stmt :=
      let_stmt
    | assign_stmt
    | expr_stmt
    | if_stmt
    | match_stmt
    | return_stmt
    ;

let_stmt      := "let" [ "mut" ] identifier [ ":" type ] "=" expr ";" ;
assign_stmt   := expr "=" expr ";" ;
expr_stmt     := expr ";" ;
if_stmt       := "if" expr "{" stmt_block "}"
                 [ "else" "{" stmt_block "}" ] ;
match_stmt    := "match" expr "{" { match_arm } "}" ;
match_arm     := pattern "=>" ( stmt_block | expr ";" ) ;
return_stmt   := "return" [ expr ] ";" ;
```

`match` arms are exhaustive: every variant of the matched type
must appear. An exhaustive `match` is required when the matched
type has variants (e.g. `Result[T, E]`). The
[`match-non-exhaustive`](../diagnostics/README.md) refusal fires
otherwise.

---

## 7. Expressions

```ebnf
expr := literal
      | identifier
      | call_expr
      | field_expr
      | index_expr
      | unary_expr
      | binary_expr
      | match_expr
      | tuple_expr
      | list_expr
      | block_expr
      | string_interp_expr
      | await_expr
      ;

literal := int_literal | float_literal | string_literal
         | "true" | "false" | "null" ;

call_expr   := expr "(" [ arg_list ] ")" ;
field_expr  := expr "." identifier ;
index_expr  := expr "[" expr "]" ;
unary_expr  := ( "-" | "!" ) expr ;
binary_expr := expr ( "+" | "-" | "*" | "/" | "%"
                     | "==" | "!=" | "<" | "<=" | ">" | ">="
                     | "&&" | "||" ) expr ;
match_expr  := "match" expr "{" { match_arm } "}" ;
tuple_expr  := "(" expr "," expr { "," expr } ")" ;
list_expr   := "[" [ expr { "," expr } ] "]" ;
block_expr  := "{" stmt_block [ expr ] "}" ;
string_interp_expr := string_literal "{" expr { "," expr } "}" ;
await_expr  := expr "." "await" "(" ")" ;
```

Evaluation order is **left to right**, the same way Cancho Core's
[`defined-behaviour.md`](https://github.com/alpibrusl/cancho/blob/main/docs/defined-behaviour.md)
defines it. A test that observes a different order is refused.

---

## 8. Types

```ebnf
type := basic_type
      | list_type
      | option_type
      | result_type
      | request_type
      | capability_type
      | event_type
      | tuple_type
      | named_type
      | "(" ")"                           (* unit *)
      ;

basic_type     := "Int" | "Float" | "Bool" | "Byte" | "String" ;
list_type      := "List" "[" type "]" ;
option_type    := "?" type | "Option" "[" type "]" ;
result_type    := "Result" "[" type "," type "]" ;
request_type   := "Request" "[" type "]" ;
capability_type := "Capability" "(" string_literal ")" ;
event_type     := "Event" "(" identifier ")" ;
tuple_type     := "(" type "," type { "," type } ")" ;
named_type     := identifier [ "<" type { "," type } ">" ] ;
```

The type checker is the source of truth for what types are
compatible. The grammar only sees them as tokens.

---

## 9. Identifiers and literals

```ebnf
identifier   := letter { letter | digit | "_" }
              | "_" { letter | digit | "_" } ;

letter       := "a".."z" | "A".."Z" | "_" ;
digit        := "0".."9" ;

int_literal  := digit { digit } ;
float_literal := digit { digit } "." digit { digit } ;
string_literal := '"' { string_char | escape } '"' ;

string_char  := ? any unicode codepoint except '"', '\', newline ? ;
escape       := "\\" ( "n" | "r" | "t" | "\\" | '"' | "0" ) ;
```

The escape set is the same six Cancho Core uses
([`AGENTS.md §6`](https://github.com/alpibrusl/cancho/blob/main/AGENTS.md)):
`\n \r \t \\ \" \0`. No `\x` or `\u`; a source file is already
UTF-8.

---

## 10. Comments

```ebnf
line_comment  := "//" { ? not-newline ? } newline ;
block_comment := "/*" { ? any ? } "*/" ;
block_comment := "/*" { ? any ? } "/*" ...   (* nested *) ;
```

Block comments nest. The same rule Cancho Core uses.

---

## 11. What Phase 1 must add to this sketch

The Phase 1 delta is small and explicit:

- A lexer producing tokens per §2, §9, §10.
- A parser producing an AST that conforms to §3–§8.
- An error recovery strategy that allows multiple diagnostics in
  one pass.
- A `tests/parser.rs` snapshot suite covering every production
  above at least once.

Anything else is Phase 2+. The grammar does not add a production
that is not in this document.

### 11.1 What Phase 1 must not add

- New keywords without a `name-reserved` test pinning them.
- New operators without a `binary-expr` snapshot test.
- New escape sequences without a `string_literal` snapshot test.
- New kinds of statements or view nodes without a design entry in
  [`docs/language-design.md`](../../docs/language-design.md).
use crate::atom_table::*;
use ordered_float::OrderedFloat;
use rug::*;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryResult {
    True,
    False,
    Matches(Vec<QueryMatch>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryMatch {
    pub bindings: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryResultLine {
    True,
    False,
    Match(BTreeMap<String, Value>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Integer(Integer),
    Rational(Rational),
    Float(OrderedFloat<f64>),
    Atom(Atom),
    String(String),
    List(Vec<Value>),
    Structure(Atom, Vec<Value>),
    Var,
}

impl From<BTreeMap<&str, Value>> for QueryMatch {
    fn from(bindings: BTreeMap<&str, Value>) -> Self {
        QueryMatch {
            bindings: bindings
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect::<BTreeMap<_, _>>(),
        }
    }
}

impl From<BTreeMap<String, Value>> for QueryMatch {
    fn from(bindings: BTreeMap<String, Value>) -> Self {
        QueryMatch { bindings }
    }
}

impl From<Vec<QueryResultLine>> for QueryResult {
    fn from(query_result_lines: Vec<QueryResultLine>) -> Self {
        // If there is only one line, and it is true or false, return that.
        if query_result_lines.len() == 1 {
            match query_result_lines[0].clone() {
                QueryResultLine::True => return QueryResult::True,
                QueryResultLine::False => return QueryResult::False,
                _ => {}
            }
        }

        // If there is at least one line with true and no matches, return true.
        if query_result_lines
            .iter()
            .any(|l| l == &QueryResultLine::True)
            && !query_result_lines.iter().any(|l| {
                if let &QueryResultLine::Match(_) = l {
                    true
                } else {
                    false
                }
            })
        {
            return QueryResult::True;
        }

        // If there is at least one match, return all matches.
        let all_matches = query_result_lines
            .into_iter()
            .filter(|l| {
                if let &QueryResultLine::Match(_) = l {
                    true
                } else {
                    false
                }
            })
            .map(|l| match l {
                QueryResultLine::Match(m) => QueryMatch::from(m),
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();

        if !all_matches.is_empty() {
            return QueryResult::Matches(all_matches);
        }

        QueryResult::False
    }
}

impl TryFrom<String> for QueryResultLine {
    type Error = ();
    fn try_from(string: String) -> Result<Self, Self::Error> {
        match string.as_str() {
            "true" => Ok(QueryResultLine::True),
            "false" => Ok(QueryResultLine::False),
            _ => Ok(QueryResultLine::Match(
                string
                    .split(",")
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(|s| -> Result<(String, Value), ()> {
                        let mut iter = s.split(" = ");
                        let key = iter.next().ok_or(())?.to_string();
                        let value = iter.next().ok_or(())?.to_string();

                        Ok((key, Value::try_from(value)?))
                    })
                    .filter_map(Result::ok)
                    .collect::<BTreeMap<_, _>>(),
            )),
        }
    }
}

impl TryFrom<String> for Value {
    type Error = ();
    fn try_from(string: String) -> Result<Self, Self::Error> {
        let trimmed = string.trim();

        if trimmed.starts_with("'") && trimmed.ends_with("'") {
            Ok(Value::String(trimmed[1..trimmed.len() - 1].into()))
        } else if trimmed.starts_with("\"") && trimmed.ends_with("\"") {
            Ok(Value::String(trimmed[1..trimmed.len() - 1].into()))
        } else if trimmed.starts_with("[") && trimmed.ends_with("]") {
            let mut iter = trimmed[1..trimmed.len() - 1].split(",");

            let mut values = vec![];

            while let Some(s) = iter.next() {
                values.push(Value::try_from(s.to_string())?);
            }

            Ok(Value::List(values))
        } else if trimmed.starts_with("{") && trimmed.ends_with("}") {
            let mut iter = trimmed[1..trimmed.len() - 1].split(",");
            let mut values = vec![];

            while let Some(value) = iter.next() {
                let items: Vec<_> = value.split(":").collect();
                if items.len() == 2 {
                    let _key = items[0].to_string();
                    let value = items[1].to_string();
                    values.push(Value::try_from(value)?);
                }
            }

            Ok(Value::Structure(atom!("{}"), values))
        } else if trimmed.starts_with("<<") && trimmed.ends_with(">>") {
            let mut iter = trimmed[2..trimmed.len() - 2].split(",");
            let mut values = vec![];

            while let Some(value) = iter.next() {
                let items: Vec<_> = value.split(":").collect();
                if items.len() == 2 {
                    let _key = items[0].to_string();
                    let value = items[1].to_string();
                    values.push(Value::try_from(value)?);
                }
            }

            Ok(Value::Structure(atom!("<<>>"), values))
        } else {
            Err(())
        }
    }
}

impl From<&str> for Value {
    fn from(str: &str) -> Self {
        Value::String(str.to_string())
    }
}

use serde::Deserialize;
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_test::{assert_de_tokens, Token};
    use std::array::IntoIter;
    use std::iter::FromIterator;

    #[test]
    fn test_deserialize() {
        let r = QueryResult {
            status: Status::Success,
            data: Some(Data {
                result_type: ResultType::Vector,
                result: vec![Metric {
                    labels: HashMap::<_, _>::from_iter(IntoIter::new([
                        (String::from("instance"), String::from("localhost:9090")),
                        (String::from("__name__"), String::from("up")),
                        (String::from("job"), String::from("prometheus")),
                    ])),
                    value: Value {
                        timestamp: 1617960600.0,
                        value: String::from("1"),
                    },
                }],
            }),
            error_type: None,
            error: None,
            warnings: None,
        };

        assert_de_tokens(
            &r,
            &[
                Token::Struct {
                    name: "QueryResult",
                    len: 2,
                },
                Token::Str("status"),
                Token::Enum { name: "Status" },
                Token::UnitVariant {
                    name: "Status",
                    variant: "Success",
                },
                Token::Str("data"),
                Token::Some,
                Token::Struct {
                    name: "Data",
                    len: 2,
                },
                Token::Str("result_type"),
                Token::Enum { name: "ResultType" },
                Token::UnitVariant {
                    name: "ResultType",
                    variant: "Vector",
                },
                Token::Str("result"),
                Token::Seq { len: Some(1) },
                Token::Struct {
                    name: "Metric",
                    len: 2,
                },
                Token::Str("metric"),
                Token::Map { len: Some(3) },
                Token::Str("instance"),
                Token::Str("localhost:9090"),
                Token::Str("__name__"),
                Token::Str("up"),
                Token::Str("job"),
                Token::Str("prometheus"),
                Token::MapEnd,
                Token::Str("value"),
                Token::Struct {
                    name: "Value",
                    len: 2,
                },
                Token::Str("timestamp"),
                Token::F64(1617960600.0),
                Token::Str("value"),
                Token::Str("1"),
                Token::StructEnd,
                Token::StructEnd,
                Token::SeqEnd,
                Token::StructEnd,
                Token::Str("error_type"),
                Token::None,
                Token::Str("error"),
                Token::None,
                Token::Str("warnings"),
                Token::None,
                Token::StructEnd,
            ],
        )
    }
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum Status {
    #[serde(alias = "success")]
    Success,
    #[serde(alias = "error")]
    Error,
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum ResultType {
    #[serde(alias = "matrix")]
    Matrix,
    #[serde(alias = "vector")]
    Vector,
    #[serde(alias = "scalar")]
    Scalar,
    #[serde(alias = "string")]
    String,
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Value {
    pub timestamp: f64,
    pub value: String,
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Metric {
    #[serde(rename = "metric")]
    pub labels: HashMap<String, String>,
    pub value: Value,
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Data {
    #[serde(alias = "resultType")]
    pub result_type: ResultType,
    pub result: Vec<Metric>,
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct QueryResult {
    pub status: Status,
    pub data: Option<Data>,
    #[serde(alias = "errorType")]
    pub error_type: Option<String>,
    pub error: Option<String>,
    pub warnings: Option<Vec<String>>,
}

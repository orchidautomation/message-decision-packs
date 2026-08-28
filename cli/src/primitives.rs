use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

macro_rules! define_primitives {
    ($(($variant:ident, $name:literal)),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub(crate) enum PrimitiveId { $($variant),+ }

        impl PrimitiveId {
            pub(crate) const ALL: [Self; define_primitives!(@count $($variant),+)] = [
                $(Self::$variant),+
            ];
            pub(crate) fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $name),+ }
            }
            pub(crate) fn names() -> Vec<&'static str> {
                Self::ALL.iter().map(|id| id.as_str()).collect()
            }
        }

        impl FromStr for PrimitiveId {
            type Err = ();
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::ALL.into_iter().find(|id| id.as_str() == value).ok_or(())
            }
        }
    };
    (@count $($variant:ident),+) => { <[()]>::len(&[$(define_primitives!(@unit $variant)),+]) };
    (@unit $variant:ident) => { () };
}

define_primitives! {
    (Actors, "actors"),
    (DecisionCriteria, "decision-criteria"),
    (SourceSignals, "source-signals"),
    (NeedsRequirements, "needs-requirements"),
    (EvidenceProof, "evidence-proof"),
    (Boundaries, "boundaries"),
    (OutputContracts, "output-contracts"),
    (RoutingJobs, "routing-jobs"),
    (Gaps, "gaps"),
    (Evals, "evals"),
}
impl fmt::Display for PrimitiveId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
impl Serialize for PrimitiveId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}
impl<'de> Deserialize<'de> for PrimitiveId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = String::deserialize(d)?;
        value
            .parse()
            .map_err(|()| serde::de::Error::custom("unknown primitive id"))
    }
}

#[cfg(test)]
mod tests {
    use super::PrimitiveId;
    use std::str::FromStr;
    const NAMES: [&str; 10] = [
        "actors",
        "decision-criteria",
        "source-signals",
        "needs-requirements",
        "evidence-proof",
        "boundaries",
        "output-contracts",
        "routing-jobs",
        "gaps",
        "evals",
    ];
    #[test]
    fn exact_vocabulary_and_order() {
        assert_eq!(PrimitiveId::ALL.len(), 10);
        assert_eq!(PrimitiveId::names(), NAMES);
    }
    #[test]
    fn strings_and_json_round_trip() {
        for id in PrimitiveId::ALL {
            assert_eq!(PrimitiveId::from_str(id.as_str()).unwrap(), id);
            let json = serde_json::to_string(&id).unwrap();
            assert_eq!(serde_json::from_str::<PrimitiveId>(&json).unwrap(), id);
        }
    }
    #[test]
    fn rejects_unknown_spellings() {
        for value in ["", "Actors", "source_signals", "gtm", "prospect"] {
            assert!(PrimitiveId::from_str(value).is_err());
        }
    }
}

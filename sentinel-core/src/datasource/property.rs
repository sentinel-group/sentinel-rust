use super::*;
use serde_json;

/// PropertyConverter func is to convert source message string to the specific property, that is, the sentinel rules.
/// if succeed to convert src, return Ok(Property)
/// if not, return the detailed error when convert src.
/// if src is None or len(src)==0, the return value is Ok()
pub type PropertyConverter<P> = fn(src: &str) -> Result<Vec<Arc<P>>>;

// `rule_json_array_parser` provide JSON as the default serialization for list of flow::Rule
pub fn rule_json_array_parser<P: SentinelRule + DeserializeOwned>(
    src: &str,
) -> Result<Vec<Arc<P>>> {
    println!("{:?}", src);
    println!("{:?}", serde_json::from_str::<Vec<P>>(src));
    let rules: Vec<P> = serde_json::from_str(src)?;
    Ok(rules.into_iter().map(|r| Arc::new(r)).collect())
}

/// PropertyUpdater func is to update the specific properties to downstream.
/// return nil if succeed to update, if not, return the error.
pub type PropertyUpdater<P> = fn(rule: Vec<Arc<P>>) -> Result<bool>;

// todo: the updater in fact is the load method now,
// the load method should be revised to a template method,

pub trait PropertyHandler<P: SentinelRule>: Send + Sync {
    // check whether the current src is consistent with last update property
    fn is_property_consistent(&mut self, rules: &[Arc<P>]) -> bool;
    // handle the current property
    fn handle(&mut self, src: Option<&String>) -> Result<bool>;
    // update sentinel rules
    fn load(&mut self, rules: Vec<Arc<P>>) -> Result<bool>;
}

/// DefaultPropertyHandler encapsulate the Converter and updater of property.
/// One DefaultPropertyHandler instance is to handle one property type.
/// DefaultPropertyHandler should check whether current property is consistent with last update property
/// converter convert the message to the specific property
/// updater update the specific property to downstream.
pub struct DefaultPropertyHandler<P: SentinelRule + PartialEq + DeserializeOwned> {
    last_update_property: Option<Vec<Arc<P>>>,
    converter: PropertyConverter<P>,
    updater: PropertyUpdater<P>,
}

impl<P: SentinelRule + PartialEq + DeserializeOwned> DefaultPropertyHandler<P> {
    pub fn new(converter: PropertyConverter<P>, updater: PropertyUpdater<P>) -> Arc<Self> {
        Arc::new(Self {
            converter,
            updater,
            last_update_property: None,
        })
    }
}

impl<P: SentinelRule + PartialEq + DeserializeOwned> PropertyHandler<P>
    for DefaultPropertyHandler<P>
{
    fn is_property_consistent(&mut self, rules: &[Arc<P>]) -> bool {
        if self.last_update_property.is_some()
            && self.last_update_property.as_ref().unwrap() == rules
        {
            true
        } else {
            self.last_update_property = Some(rules.to_vec());
            false
        }
    }

    fn handle(&mut self, src: Option<&String>) -> Result<bool> {
        match src {
            Some(src) => {
                let rules = (self.converter)(src)?;
                let is_the_same = self.is_property_consistent(&rules);
                if is_the_same {
                    return Ok(false);
                }
                (self.updater)(rules)
            }
            None => (self.updater)(Vec::new()),
        }
    }

    fn load(&mut self, rules: Vec<Arc<P>>) -> Result<bool> {
        (self.updater)(rules)
    }
}

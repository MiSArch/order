use std::any::type_name;

// Matches MongoDB collection name to a type T.
fn infer_collection_name<T>() -> String {
    let type_name = type_name::<T>();
    type_name.find(pattern)
}

fn extract_prefixless_type(input: &str) -> IResult<&str, &str> {
    
}
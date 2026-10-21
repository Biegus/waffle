use std::collections::HashMap;

use crate::helper;

fn get_string_list(t:&[&str]) -> Vec<String>{
    return t.into_iter().map(|t|t.to_string()).collect()
}
fn get_string_hashmap(t: &[(&str, &str)]) -> HashMap<String,String>{
    return t.into_iter().map(|(a,b)| (a.to_string(), b.to_string())).collect();
}


#[test]
fn test_get_pparams(){
    assert_eq!(helper::get_pparams("entries/<id>/parts/<part_id>/get"),  get_string_list(&["id","part_id"]))
}
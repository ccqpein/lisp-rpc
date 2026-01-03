use lisp_rpc_rust_parser::{Parser, TypeValue, data::*}; // import the data module

fn main() {
    // client send some data
    // this data is bad
    let client_data_illegal = Data::new(
        "rpc call",
        [
            ("version", &1_i32 as &dyn IntoData),
            ("aa", &IntoData::into_rpc_data(&2_i32)),
        ]
        .into_iter(),
    );

    match client_data_illegal {
        Ok(_) => (),
        Err(e) => println!("data name cannot have the space inside: {}", e.to_string()),
    }

    // this is the legal one
    let client_data = Data::new(
        "rpc-call",
        [
            ("version", &1_i32 as &dyn IntoData),
            ("aa", &IntoData::into_rpc_data(&2_i32)),
        ]
        .into_iter(),
    );

    // then can send this data to server
    let raw_data = client_data.unwrap().to_string();
    println!("raw_data is {raw_data}\n");

    // server side
    // server can parse the data send from client and return the response
    let client_request_data = match Data::from_str(&Default::default(), &raw_data).unwrap() {
        Data::Data(expr_data) => expr_data, // root data has to be Expr data
        _ => panic!(),
    };

    // check the msg name
    let _ = client_request_data.get_name();

    let &Data::Value(TypeValue::Number(version_v)) = client_request_data.get("version").unwrap()
    else {
        panic!()
    };

    let &Data::Value(TypeValue::Number(aa_v)) = client_request_data.get("aa").unwrap() else {
        panic!()
    };

    let _ = client_request_data.get("bb");

    // server side for some reason want to format str to send data
    let server_response_data = Data::from_str(
        &Parser::new().config_read_number(true),
        &format!("(response :args '(1 2) :result {})", version_v + aa_v),
    )
    .unwrap();

    println!("server_response_data is\n{:?}\n", server_response_data);

    // lets say the server send this to client
    println!(
        "raw server_response_data is \n{:?}\n",
        server_response_data.to_string()
    );

    // client get the response
    let response_client_get = Data::from_root_str(&server_response_data.to_string(), None).unwrap();
    println!(
        "response is:\n{:?}\n\nraw string data is\n{}\n\nresult is\n{:?}\n",
        response_client_get,
        response_client_get.to_string(),
        response_client_get.get("result")
    )
}

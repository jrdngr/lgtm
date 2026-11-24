//! Claude Code runs `lgtm agent hook`, which records the agent's turns and, in an lgterm terminal,
//! passes the hook on to lgterm.

pub fn hook() -> i32 {
    let input = crate::turn_hook::read_input();
    tell_lgterm(&input);
    crate::turn_hook::run_on(&input)
}

/// Passes a hook on to lgterm when this runs in one of its terminals, so it can show the
/// agent's status, and prints what lgterm has for the hook to say. Waits briefly for lgterm
/// to take it, so hooks arrive in order.
fn tell_lgterm(input: &str) {
    use lgterm_proto::{Reply, Request};

    let Some((term, socket)) = lgterm_proto::here() else {
        return;
    };
    let Ok(event) = serde_json::from_str(input) else {
        return;
    };
    let hook = Request::Hook {
        term: term.to_string(),
        event,
    };
    let wait = std::time::Duration::from_secs(2);
    if let Ok(Reply::Ok(out)) = lgterm_proto::call(&socket, &hook, Some(wait))
        && !out.is_empty()
    {
        println!("{out}");
    }
}

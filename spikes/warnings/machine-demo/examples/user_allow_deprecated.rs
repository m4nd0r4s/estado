//! Where does a user's `#[allow(deprecated)]` hide W0001?
use machine_demo::machine;

// 1. On the enum: the generated `const _` is a sibling, so NOT hidden.
#[allow(deprecated)]
#[machine]
enum OnEnum {
    A,
    #[unreachable_demo]
    OnEnumOrphan,
}

// 2. On an enclosing module: hidden.
#[allow(deprecated)]
mod hidden_by_module {
    use machine_demo::machine;
    #[machine]
    pub enum InModule {
        A,
        #[unreachable_demo]
        InModuleOrphan,
    }
}

// 3. Inside a function body, with generics-free enum: still precise.
fn in_function() {
    #[machine]
    enum InFunction {
        A,
        #[unreachable_demo]
        InFunctionOrphan,
    }
    let _ = InFunction::A;
}

// 4. Silenced through the machine argument.
#[machine(allow(W0001))]
enum Silenced {
    A,
    #[unreachable_demo]
    SilencedOrphan,
}

fn main() {
    let _ = (
        OnEnum::A,
        OnEnum::OnEnumOrphan,
        hidden_by_module::InModule::A,
        Silenced::A,
    );
    let _ = Silenced::SilencedOrphan;
    in_function();
}

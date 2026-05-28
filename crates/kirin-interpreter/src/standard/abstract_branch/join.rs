use kirin_ir::Product;

use crate::{AbstractValue, InterpreterError, StandardCompletion};

pub(super) fn join_standard_completion<V>(
    left: StandardCompletion<V>,
    right: StandardCompletion<V>,
) -> Result<StandardCompletion<V>, InterpreterError>
where
    V: AbstractValue,
{
    match (left, right) {
        (
            StandardCompletion::FunctionReturned(left),
            StandardCompletion::FunctionReturned(right),
        ) => {
            if left.len() != right.len() {
                return Err(InterpreterError::Custom(
                    "abstract branch paths returned different arities",
                ));
            }
            Ok(StandardCompletion::FunctionReturned(
                left.iter()
                    .zip(right.iter())
                    .map(|(left, right)| left.join(right))
                    .collect::<Product<_>>(),
            ))
        }
        (StandardCompletion::BlockDone, StandardCompletion::BlockDone) => {
            Ok(StandardCompletion::BlockDone)
        }
        (StandardCompletion::RegionDone, StandardCompletion::RegionDone) => {
            Ok(StandardCompletion::RegionDone)
        }
        (StandardCompletion::GraphDone, StandardCompletion::GraphDone) => {
            Ok(StandardCompletion::GraphDone)
        }
        _ => Err(InterpreterError::Custom(
            "abstract branch paths produced incompatible completions",
        )),
    }
}

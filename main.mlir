type = FLAIRKernelFuncType
new_type = BufStateKernelFuncType

fn @kernel<[total_buf_state = 4]>(%x: i32) -> !pulse {
    buffer.alloca() : !buffer
    %buf = buffer.define_state(index = 1)
    some_pulse_instruction
}

fn @main() [total_buf_state = 8] {
    call<1, 3, 5, 7> @kernel(%arg0 : i32) : !pulse
    call<2, 4, 6, 8> @kernel(%arg0 : i32) : !pulse
}

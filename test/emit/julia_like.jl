function _callable_julia_like(ssa_x, ssa_y)
    @label block_0
    ssa_0 = 0:1:ssa_x
    local ssa_y_1
    local ssa_y_2
    for ssa_i in ssa_0
        ssa_y_3 = ssa_y
        ssa_y_4 = ssa_y
        ssa_1 = 0:1:ssa_y_3
        local ssa_i_1
        local ssa_i_2
        for ssa_j in ssa_1
            ssa_i_3 = ssa_i
            ssa_i_4 = ssa_i
            ssa_2 = (ssa_i_3 == 0)
            if ssa_2
                ssa_3 = ssa_2
                @info "Hello"
            else
                ssa_4 = ssa_2
                @info "World"
            end
            ssa_i_1 = ssa_i_3
            ssa_i_2 = ssa_i_3
        end
        ssa_y_1 = ssa_y_3
        ssa_y_2 = ssa_y_3
    end
    ssa_5 = (ssa_x + ssa_y_2)
    ssa_6 = _callable_some_arith(ssa_5, 4.0)
    return ssa_6
end

function _callable_some_arith(ssa_x, ssa_y)
    @label block_0
    ssa_0 = (ssa_x + ssa_y)
    return ssa_0
end

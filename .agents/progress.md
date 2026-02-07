Replaced StageInfo symbol storage from Arc<RefCell<InternTable<...>>> to direct InternTable access and updated all symbol table call sites to use direct references and mutable references.

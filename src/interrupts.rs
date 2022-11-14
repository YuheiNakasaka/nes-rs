pub mod interrupts {
    #[derive(PartialEq, Eq)]
    pub enum InterruptType {
        NMI,
    }

    #[derive(PartialEq, Eq)]
    pub struct Interrupt {
        pub itype: InterruptType,
        pub vector_addr: u16,
        pub b_flag_mask: u8,
        pub cpu_cycles: u8,
    }

    pub const NMI: Interrupt = Interrupt {
        itype: InterruptType::NMI,
        vector_addr: 0xfffa,
        b_flag_mask: 0b0010_0000,
        cpu_cycles: 2,
    };
}

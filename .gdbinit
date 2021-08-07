define target hookpost-remote

# set backtrace limit to not have infinite backtrace loops
set backtrace limit 32

# detect unhandled exceptions, hard faults and panics
#break DefaultHandler
break HardFault
break rust_begin_unwind

# *try* to stop at the user entry point (it might be gone due to inlining)
# break main

target extended-remote :3333

set print asm-demangle on

monitor arm semihosting enable

#end

set br pend on
set pr asm on
han SIGSEGV noprint
b ctor
dis
b __libc_start_main
dis
b procedure_linkage_override
comm
dis
find/b1 procedure_linkage_override, +0x1000, 0xc3
set $plo_ret = $_
b *$plo_ret
comm
x/a $rsp
c
end
c
end
c
end
c
end

set br pend on
set pr asm on
han SIGSEGV noprint
b libgotcha_traceglobal
comm
p/a $rsi
c
end

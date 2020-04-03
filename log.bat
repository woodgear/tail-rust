set count=0
:loop
set /a count=%count%+1
echo -%count%- >> test.log
sleep 1
if %count% neq 10000 goto loop
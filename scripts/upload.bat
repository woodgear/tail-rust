call .\scripts\build-xp.bat
if %errorlevel% neq 0 exit /b %errorlevel%
echo "copy"
set TO=\\192.168.122.1\smb\tail.exe
set FROM=.\target\release\tail.exe
copy /Y %FROM% %TO% 
if %errorlevel% neq 0 exit /b %errorlevel%

md5sum.exe %FROM% 
md5sum.exe %TO% 
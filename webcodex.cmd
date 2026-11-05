@echo off
python -m webcodex %*
exit /
=======
@echo off
python -m webcodex %*
exit /b %errorlevel%

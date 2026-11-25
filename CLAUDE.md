# Навигация по коду

Для вопросов об архитектуре и связях между модулями сначала спрашивай граф, а не обходи файлы:

    C:\Users\bmwto\graphify-venv\Scripts\graphify.exe query "<вопрос>" --budget 1500

Если граф устарел после правок кода: `graphify.exe update .` (локально, без LLM, токены не тратит).
Не читай graphify-out/GRAPH_REPORT.md целиком. Папка graphify-out/ в git не попадает.

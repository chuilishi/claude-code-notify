// ============================================================================
// ToastWindow - Native C++ Toast Notification for Claude Code
// ============================================================================
//
// A lightweight native Windows toast notification system designed for Claude Code.
// Uses Claude Code hooks to capture window state at prompt submit time, then
// displays non-intrusive notifications when Claude completes a task.
//
// ARCHITECTURE:
//   Both hooks receive JSON via stdin containing session_id for state isolation.
//   State is stored in %TEMP%\claude-notify-{session_id}.txt
//
// USAGE:
//   ToastWindow.exe --save      Save window state (UserPromptSubmit hook)
//   ToastWindow.exe --notify    Show notification (Stop hook)
//   ToastWindow.exe --input     Show input-required notification (Notification hook)
//
// FEATURES:
//   - Session-based state isolation (multiple Claude instances supported)
//   - Telegram-style notification stacking
//   - Windows Terminal tab switching via UI Automation
//   - Caller app icon saved at prompt time
//   - Mouse hover pauses all toast timers
//   - Non-focus-stealing display
//
// COMPILE (MSVC):
//   cl /EHsc /O2 /MT ToastWindow.cpp /Fe:ToastWindow.exe ^
//      ole32.lib oleaut32.lib shell32.lib user32.lib gdi32.lib winmm.lib uuid.lib
//
// ============================================================================

#define WIN32_LEAN_AND_MEAN
#define UNICODE
#define _UNICODE

#include <windows.h>
#include <shellapi.h>
#include <tlhelp32.h>
#include <shlobj.h>
#include <mmsystem.h>
#include <initguid.h>
#include <uiautomation.h>

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>
#include <algorithm>
#include <cwchar>
#include <io.h>
#include <fcntl.h>

// ============================================================================
// Global Configuration
// ============================================================================

// Toast content
static std::wstring g_title = L"Claude Code";
static std::wstring g_message = L"Task completed";
static std::wstring g_soundFile;
static std::wstring g_fontFile;
static std::wstring g_fontFamily = L"Segoe UI";
static bool g_fontLoaded = false;

#define FR_PRIVATE 0x10

// Paths
static std::wstring g_logFile;
static std::wstring g_defaultIconPath;
static std::wstring g_className;
static std::wstring g_savedIconPath;  // Icon path saved from --save mode

// Toast dimensions and timing
static int g_windowWidth = 300;
static int g_windowHeight = 80;
static int g_iconSize = 48;
static int g_iconPadding = 16;
static int g_displayMs = 3000;
static int g_fadeMs = 1000;
static int g_fadeStep = 15;
static BYTE g_alpha = 230;

// Runtime state
static bool g_debug = false;
static bool g_clicked = false;
static bool g_inputMode = false;  // True for input-required notifications (yellow theme)
static HWND g_immediateHwnd = NULL;  // Captured at program start before any delay

// Window handles
static HWND g_hwnd = NULL;
static HICON g_appIcon = NULL;
static HWND g_targetWindowHandle = NULL;

// Windows Terminal state
static HWND g_wtWindowHandle = NULL;
static std::wstring g_wtSavedRuntimeId;
static IUIAutomation* g_pAutomation = NULL;

// Timer IDs
static const UINT_PTR TIMER_FADE = 1;
static const UINT_PTR TIMER_START_FADE = 2;
static const UINT_PTR TIMER_REPOSITION = 3;
static const UINT_PTR TIMER_CHECK_BOTTOM = 4;

// Close button
static const int CLOSE_BUTTON_SIZE = 20;
static const int CLOSE_BUTTON_MARGIN = 6;

// Mouse tracking
static bool g_mouseInside = false;
static bool g_isFading = false;

// Stacking - fixed class name for all toast windows
static const wchar_t* TOAST_CLASS_NAME = L"ClaudeCodeToast";
static int g_targetY = 0;  // Target Y position for smooth animation
static bool g_isBottomToast = false;  // Only bottom toast starts fade timer
static RECT g_workArea = {0};  // Cached work area
static UINT g_taskbarEdge = ABE_BOTTOM;  // Cached taskbar edge

// Custom messages for inter-toast communication
static const UINT WM_TOAST_CLOSING = WM_USER + 100;
static const UINT WM_TOAST_CHECK_POSITION = WM_USER + 101;
static const UINT WM_TOAST_PAUSE_TIMER = WM_USER + 102;    // wParam: 1=pause, 0=resume

// ============================================================================
// Logging
// ============================================================================

static void Log(const wchar_t* fmt, ...) {
    if (!g_debug && g_logFile.empty()) return;

    wchar_t buf[2048];
    va_list args;
    va_start(args, fmt);
    vswprintf(buf, sizeof(buf)/sizeof(buf[0]), fmt, args);
    va_end(args);

    if (g_debug) {
        int utf8Len = WideCharToMultiByte(CP_UTF8, 0, buf, -1, NULL, 0, NULL, NULL);
        if (utf8Len > 0) {
            char* utf8Buf = (char*)malloc(utf8Len);
            if (utf8Buf) {
                WideCharToMultiByte(CP_UTF8, 0, buf, -1, utf8Buf, utf8Len, NULL, NULL);
                printf("%s\n", utf8Buf);
                free(utf8Buf);
            }
        }
        fflush(stdout);
    }

    if (!g_logFile.empty()) {
        FILE* f = _wfopen(g_logFile.c_str(), L"a, ccs=UTF-8");
        if (f) {
            fwprintf(f, L"%ls\n", buf);
            fclose(f);
        }
    }
}

// ============================================================================
// Process Utilities
// ============================================================================

static DWORD GetParentProcessId(DWORD processId) {
    HANDLE hSnapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if (hSnapshot == INVALID_HANDLE_VALUE) return 0;

    PROCESSENTRY32W pe;
    pe.dwSize = sizeof(pe);
    if (Process32FirstW(hSnapshot, &pe)) {
        do {
            if (pe.th32ProcessID == processId) {
                CloseHandle(hSnapshot);
                return pe.th32ParentProcessID;
            }
        } while (Process32NextW(hSnapshot, &pe));
    }

    CloseHandle(hSnapshot);
    return 0;
}

static std::wstring GetProcessExePath(DWORD processId) {
    HANDLE hProcess = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, processId);
    if (!hProcess) return L"";

    wchar_t path[MAX_PATH];
    DWORD size = MAX_PATH;
    if (QueryFullProcessImageNameW(hProcess, 0, path, &size)) {
        CloseHandle(hProcess);
        return std::wstring(path);
    }

    CloseHandle(hProcess);
    return L"";
}

static std::wstring GetFileNameWithoutExt(const std::wstring& path) {
    size_t pos = path.find_last_of(L"\\/");
    std::wstring name = (pos == std::wstring::npos) ? path : path.substr(pos + 1);
    pos = name.find_last_of(L'.');
    if (pos != std::wstring::npos) name = name.substr(0, pos);
    return name;
}

static std::wstring ToLower(const std::wstring& s) {
    std::wstring result = s;
    std::transform(result.begin(), result.end(), result.begin(), towlower);
    return result;
}

// ============================================================================
// Icon Extraction (simplified - called at save time)
// ============================================================================

// Shell processes to skip when finding caller app (for icon extraction)
static const wchar_t* g_skipList[] = {
    // Windows shells
    L"cmd", L"powershell", L"pwsh", L"conhost", L"explorer",
    // Unix shells (WSL/Git Bash)
    L"bash", L"zsh", L"fish", L"sh", L"wsl", L"mintty",
    // Git
    L"git", L"git-bash",
    // JavaScript/TypeScript runtimes
    L"node", L"deno", L"bun", L"npx", L"ts-node", L"npm", L"yarn", L"pnpm",
    // Python
    L"python", L"python3", L"uv", L"pip", L"poetry", L"pdm",
    // Other languages
    L"ruby", L"java", L"dotnet", L"php", L"go", L"cargo", L"rustc", L"perl", L"lua",
    // Claude CLI
    L"claude",
    // Remote/containers
    L"ssh", L"docker", L"podman",
    NULL
};


/// Finds the caller app's executable path by walking up the process tree.
/// Called during --save when the foreground window is guaranteed correct.
static std::wstring FindCallerExePath() {
    static const wchar_t* knownApps[] = {
        // VS Code variants
        L"code", L"code-insiders", L"codium", L"cursor", L"windsurf",
        // JetBrains IDEs
        L"idea", L"idea64", L"webstorm", L"webstorm64", L"pycharm", L"pycharm64",
        L"rider", L"rider64", L"goland", L"goland64", L"clion", L"clion64",
        // Terminal emulators
        L"windowsterminal", L"wt", L"conemu", L"conemu64",
        L"tabby", L"wezterm", L"wezterm-gui",
        NULL
    };

    DWORD pid = GetCurrentProcessId();

    for (int i = 0; i < 10; i++) {
        DWORD parentPid = GetParentProcessId(pid);
        if (parentPid == 0 || parentPid == pid) break;

        std::wstring exePath = GetProcessExePath(parentPid);
        if (exePath.empty()) {
            pid = parentPid;
            continue;
        }

        std::wstring exeName = ToLower(GetFileNameWithoutExt(exePath));

        // Check if known app
        for (int j = 0; knownApps[j] != NULL; j++) {
            if (exeName == knownApps[j] || exeName.find(std::wstring(knownApps[j]) + L"-") == 0) {
                return exePath;
            }
        }

        // Check if should skip (exact match only)
        bool skip = false;
        for (int j = 0; g_skipList[j] != NULL; j++) {
            if (exeName == g_skipList[j]) {
                skip = true;
                break;
            }
        }

        if (!skip) {
            return exePath;  // Unknown but valid caller
        }

        pid = parentPid;
    }

    return L"";
}

// ============================================================================
// UI Automation Helpers
// ============================================================================

static void InitUIAutomation() {
    if (g_pAutomation) return;

    HRESULT hr = CoCreateInstance(
        CLSID_CUIAutomation, NULL, CLSCTX_INPROC_SERVER,
        IID_IUIAutomation, (void**)&g_pAutomation
    );

    if (FAILED(hr)) {
        Log(L"[DEBUG] Failed to create IUIAutomation: 0x%08X", (unsigned)hr);
        g_pAutomation = NULL;
    }
}

static std::wstring GetRuntimeIdString(IUIAutomationElement* element) {
    if (!element) return L"";

    SAFEARRAY* psa = NULL;
    HRESULT hr = element->GetRuntimeId(&psa);
    if (FAILED(hr) || !psa) return L"";

    LONG lbound, ubound;
    SafeArrayGetLBound(psa, 1, &lbound);
    SafeArrayGetUBound(psa, 1, &ubound);

    std::wstring result;
    for (LONG i = lbound; i <= ubound; i++) {
        int val;
        SafeArrayGetElement(psa, &i, &val);
        if (!result.empty()) result += L".";
        result += std::to_wstring(val);
    }

    SafeArrayDestroy(psa);
    return result;
}

static bool IsWindowsTerminalWindow(HWND hwnd) {
    if (!hwnd) return false;
    wchar_t className[256];
    if (GetClassNameW(hwnd, className, 256) > 0) {
        return wcscmp(className, L"CASCADIA_HOSTING_WINDOW_CLASS") == 0;
    }
    return false;
}

// ============================================================================
// State File Management
// ============================================================================

// Global to store user prompt from stdin
static std::wstring g_userPrompt;

/// Simple JSON string value extractor (no external library needed)
static std::wstring ExtractJsonStringValue(const std::string& json, const char* key) {
    std::string searchKey = "\"";
    searchKey += key;
    searchKey += "\"";

    size_t keyPos = json.find(searchKey);
    if (keyPos == std::string::npos) return L"";

    size_t colonPos = json.find(':', keyPos + searchKey.length());
    if (colonPos == std::string::npos) return L"";

    // Skip whitespace
    size_t valueStart = colonPos + 1;
    while (valueStart < json.length() && (json[valueStart] == ' ' || json[valueStart] == '\t' || json[valueStart] == '\n' || json[valueStart] == '\r')) {
        valueStart++;
    }

    if (valueStart >= json.length() || json[valueStart] != '"') return L"";
    valueStart++; // Skip opening quote

    // Find closing quote (handle escape sequences)
    std::string value;
    for (size_t i = valueStart; i < json.length(); i++) {
        if (json[i] == '\\' && i + 1 < json.length()) {
            i++;
            switch (json[i]) {
                case 'n': value += '\n'; break;
                case 'r': value += '\r'; break;
                case 't': value += '\t'; break;
                case '"': value += '"'; break;
                case '\\': value += '\\'; break;
                default: value += json[i]; break;
            }
        } else if (json[i] == '"') {
            break;
        } else {
            value += json[i];
        }
    }

    // Convert UTF-8 to wide string
    int wlen = MultiByteToWideChar(CP_UTF8, 0, value.c_str(), -1, NULL, 0);
    if (wlen <= 0) return L"";
    std::wstring wvalue(wlen - 1, L'\0');
    MultiByteToWideChar(CP_UTF8, 0, value.c_str(), -1, &wvalue[0], wlen);
    return wvalue;
}

/// Read JSON from stdin (non-blocking, with timeout)
static std::string ReadStdinJson() {
    std::string result;

    // Set stdin to binary mode
    _setmode(_fileno(stdin), _O_BINARY);

    // Read all available data from stdin
    char buffer[4096];
    while (true) {
        size_t bytesRead = fread(buffer, 1, sizeof(buffer), stdin);
        if (bytesRead == 0) break;
        result.append(buffer, bytesRead);
        if (bytesRead < sizeof(buffer)) break;
    }

    return result;
}

// Session ID for state file isolation (from Claude Code's stdin JSON)
static std::wstring g_sessionId;

/// Gets state file path based on session ID.
/// Session ID is extracted from Claude Code's stdin JSON and persists across hooks.
static std::wstring GetStateFilePath(const std::wstring& sessionId) {
    wchar_t tempPath[MAX_PATH];
    GetTempPathW(MAX_PATH, tempPath);
    return std::wstring(tempPath) + L"claude-notify-" + sessionId + L".txt";
}

/// Saves current window state to file.
/// Called by UserPromptSubmit hook - foreground window is guaranteed to be the terminal.
static void SaveState() {
    // Read JSON from stdin to get session_id and user prompt
    std::string jsonInput = ReadStdinJson();
    g_sessionId = ExtractJsonStringValue(jsonInput, "session_id");
    std::wstring userPrompt = ExtractJsonStringValue(jsonInput, "prompt");

    if (g_sessionId.empty()) {
        Log(L"[DEBUG] No session_id in stdin JSON, cannot save state");
        return;
    }

    Log(L"[DEBUG] Session ID: %ls", g_sessionId.c_str());
    Log(L"[DEBUG] User prompt: %ls", userPrompt.c_str());

    // Use HWND captured at program start (before stdin read delay)
    HWND hwnd = g_immediateHwnd;
    if (!hwnd || !IsWindow(hwnd)) {
        hwnd = GetForegroundWindow();  // Fallback
    }
    std::wstring stateFile = GetStateFilePath(g_sessionId);

    Log(L"[DEBUG] Saving state to: %ls", stateFile.c_str());
    Log(L"[DEBUG] Foreground HWND: %lld", (long long)(intptr_t)hwnd);

    FILE* f = _wfopen(stateFile.c_str(), L"w, ccs=UTF-8");
    if (!f) {
        Log(L"[DEBUG] Failed to open state file for writing");
        return;
    }

    // Line 1: HWND
    fwprintf(f, L"%lld\n", (long long)(intptr_t)hwnd);

    // Line 2: RuntimeId (if Windows Terminal)
    std::wstring runtimeId;
    if (IsWindowsTerminalWindow(hwnd)) {
        Log(L"[DEBUG] Window is Windows Terminal");

        IUIAutomation* pAutomation = NULL;
        HRESULT hr = CoCreateInstance(
            CLSID_CUIAutomation, NULL, CLSCTX_INPROC_SERVER,
            IID_IUIAutomation, (void**)&pAutomation
        );

        if (SUCCEEDED(hr) && pAutomation) {
            IUIAutomationElement* pElement = NULL;
            hr = pAutomation->ElementFromHandle(hwnd, &pElement);

            if (SUCCEEDED(hr) && pElement) {
                VARIANT varControlType;
                VariantInit(&varControlType);
                varControlType.vt = VT_I4;
                varControlType.lVal = UIA_TabItemControlTypeId;

                IUIAutomationCondition* pCondition = NULL;
                pAutomation->CreatePropertyCondition(UIA_ControlTypePropertyId, varControlType, &pCondition);
                VariantClear(&varControlType);

                if (pCondition) {
                    IUIAutomationElementArray* pTabs = NULL;
                    hr = pElement->FindAll(TreeScope_Descendants, pCondition, &pTabs);
                    pCondition->Release();

                    if (SUCCEEDED(hr) && pTabs) {
                        int tabCount = 0;
                        pTabs->get_Length(&tabCount);
                        Log(L"[DEBUG] Found %d tabs", tabCount);

                        for (int i = 0; i < tabCount; i++) {
                            IUIAutomationElement* pTab = NULL;
                            pTabs->GetElement(i, &pTab);
                            if (!pTab) continue;

                            IUIAutomationSelectionItemPattern* pPattern = NULL;
                            hr = pTab->GetCurrentPatternAs(UIA_SelectionItemPatternId,
                                IID_IUIAutomationSelectionItemPattern, (void**)&pPattern);

                            if (SUCCEEDED(hr) && pPattern) {
                                BOOL isSelected = FALSE;
                                pPattern->get_CurrentIsSelected(&isSelected);
                                pPattern->Release();

                                if (isSelected) {
                                    runtimeId = GetRuntimeIdString(pTab);
                                    Log(L"[DEBUG] Selected tab RuntimeId: %ls", runtimeId.c_str());
                                    pTab->Release();
                                    break;
                                }
                            }
                            pTab->Release();
                        }
                        pTabs->Release();
                    }
                }
                pElement->Release();
            }
            pAutomation->Release();
        }
    }
    fwprintf(f, L"%ls\n", runtimeId.c_str());

    // Line 3: Caller exe path (for icon)
    std::wstring callerExe = FindCallerExePath();
    Log(L"[DEBUG] Caller exe: %ls", callerExe.c_str());
    fwprintf(f, L"%ls\n", callerExe.c_str());

    // Line 4: User prompt (truncated for display)
    fwprintf(f, L"%ls\n", userPrompt.c_str());

    fclose(f);
    Log(L"[DEBUG] State saved successfully");
}

/// Loads saved state from file.
static void LoadState(const std::wstring& sessionId) {
    std::wstring stateFile = GetStateFilePath(sessionId);

    Log(L"[DEBUG] Loading state from: %ls", stateFile.c_str());

    FILE* f = _wfopen(stateFile.c_str(), L"r, ccs=UTF-8");
    if (!f) {
        Log(L"[DEBUG] State file not found");
        return;
    }

    wchar_t line[2048];

    // Line 1: HWND
    if (fgetws(line, 2048, f)) {
        size_t len = wcslen(line);
        while (len > 0 && (line[len-1] == L'\n' || line[len-1] == L'\r')) line[--len] = L'\0';

        HWND hwnd = (HWND)(intptr_t)_wtoi64(line);
        if (hwnd && IsWindow(hwnd)) {
            g_targetWindowHandle = hwnd;
            if (IsWindowsTerminalWindow(hwnd)) {
                g_wtWindowHandle = hwnd;
            }
            Log(L"[DEBUG] Loaded HWND: %lld", (long long)(intptr_t)hwnd);
        }
    }

    // Line 2: RuntimeId
    if (fgetws(line, 2048, f)) {
        size_t len = wcslen(line);
        while (len > 0 && (line[len-1] == L'\n' || line[len-1] == L'\r')) line[--len] = L'\0';

        if (wcslen(line) > 0) {
            g_wtSavedRuntimeId = line;
            Log(L"[DEBUG] Loaded RuntimeId: %ls", g_wtSavedRuntimeId.c_str());
        }
    }

    // Line 3: Icon path
    if (fgetws(line, 2048, f)) {
        size_t len = wcslen(line);
        while (len > 0 && (line[len-1] == L'\n' || line[len-1] == L'\r')) line[--len] = L'\0';

        if (wcslen(line) > 0) {
            g_savedIconPath = line;
            Log(L"[DEBUG] Loaded icon path: %ls", g_savedIconPath.c_str());
        }
    }

    // Line 4: User prompt
    if (fgetws(line, 2048, f)) {
        size_t len = wcslen(line);
        while (len > 0 && (line[len-1] == L'\n' || line[len-1] == L'\r')) line[--len] = L'\0';

        if (wcslen(line) > 0) {
            g_userPrompt = line;
            Log(L"[DEBUG] Loaded user prompt: %ls", g_userPrompt.c_str());
        }
    }

    fclose(f);
}

// ============================================================================
// Window Activation
// ============================================================================

static void TryAltKeyTrick() {
    keybd_event(VK_MENU, 0, KEYEVENTF_EXTENDEDKEY, 0);
    keybd_event(VK_MENU, 0, KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP, 0);
    Sleep(50);
}

/// Switches to saved Windows Terminal tab using RuntimeId.
static void SwitchToWindowsTerminalTab() {
    if (g_wtSavedRuntimeId.empty() || !g_wtWindowHandle) {
        Log(L"[DEBUG] No WT tab info to switch to");
        return;
    }

    Log(L"[DEBUG] Switching to WT tab (runtimeId=%ls)", g_wtSavedRuntimeId.c_str());

    if (!IsWindow(g_wtWindowHandle)) {
        Log(L"[DEBUG] WT window no longer exists");
        return;
    }

    // Bring window to foreground
    if (IsIconic(g_wtWindowHandle)) {
        ShowWindow(g_wtWindowHandle, SW_RESTORE);
    }

    AllowSetForegroundWindow(ASFW_ANY);
    TryAltKeyTrick();

    HWND fgWnd = GetForegroundWindow();
    DWORD fgThread = GetWindowThreadProcessId(fgWnd, NULL);
    DWORD curThread = GetCurrentThreadId();
    DWORD targetThread = GetWindowThreadProcessId(g_wtWindowHandle, NULL);

    AttachThreadInput(curThread, fgThread, TRUE);
    AttachThreadInput(curThread, targetThread, TRUE);

    SetWindowPos(g_wtWindowHandle, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW);
    BringWindowToTop(g_wtWindowHandle);
    SwitchToThisWindow(g_wtWindowHandle, TRUE);
    SetForegroundWindow(g_wtWindowHandle);

    AttachThreadInput(curThread, targetThread, FALSE);
    AttachThreadInput(curThread, fgThread, FALSE);

    // Find and select tab by RuntimeId
    InitUIAutomation();
    if (!g_pAutomation) return;

    IUIAutomationElement* pWtElement = NULL;
    HRESULT hr = g_pAutomation->ElementFromHandle(g_wtWindowHandle, &pWtElement);
    if (FAILED(hr) || !pWtElement) return;

    VARIANT varControlType;
    VariantInit(&varControlType);
    varControlType.vt = VT_I4;
    varControlType.lVal = UIA_TabItemControlTypeId;

    IUIAutomationCondition* pTabCondition = NULL;
    g_pAutomation->CreatePropertyCondition(UIA_ControlTypePropertyId, varControlType, &pTabCondition);
    VariantClear(&varControlType);

    if (!pTabCondition) {
        pWtElement->Release();
        return;
    }

    IUIAutomationElementArray* pTabs = NULL;
    hr = pWtElement->FindAll(TreeScope_Descendants, pTabCondition, &pTabs);
    pTabCondition->Release();
    pWtElement->Release();

    if (FAILED(hr) || !pTabs) return;

    int tabCount = 0;
    pTabs->get_Length(&tabCount);

    for (int i = 0; i < tabCount; i++) {
        IUIAutomationElement* pTab = NULL;
        pTabs->GetElement(i, &pTab);
        if (!pTab) continue;

        std::wstring tabRuntimeId = GetRuntimeIdString(pTab);
        if (tabRuntimeId == g_wtSavedRuntimeId) {
            IUIAutomationSelectionItemPattern* pPattern = NULL;
            hr = pTab->GetCurrentPatternAs(UIA_SelectionItemPatternId,
                IID_IUIAutomationSelectionItemPattern, (void**)&pPattern);

            if (SUCCEEDED(hr) && pPattern) {
                pPattern->Select();
                Log(L"[DEBUG] Tab switched successfully");
                pPattern->Release();
            }
            pTab->Release();
            break;
        }
        pTab->Release();
    }
    pTabs->Release();
}

/// Activates the saved window.
static void ActivateWindow() {
    // Windows Terminal with tab info
    if (g_wtWindowHandle && !g_wtSavedRuntimeId.empty()) {
        Log(L"[DEBUG] Using Windows Terminal tab switching");
        SwitchToWindowsTerminalTab();
        return;
    }

    // Regular window
    if (!g_targetWindowHandle || !IsWindow(g_targetWindowHandle)) {
        Log(L"[DEBUG] No valid target window");
        return;
    }

    Log(L"[DEBUG] Activating window: %lld", (long long)(intptr_t)g_targetWindowHandle);

    AllowSetForegroundWindow(ASFW_ANY);

    if (IsIconic(g_targetWindowHandle)) {
        ShowWindow(g_targetWindowHandle, SW_RESTORE);
    }

    TryAltKeyTrick();

    HWND fgWnd = GetForegroundWindow();
    DWORD fgThread = GetWindowThreadProcessId(fgWnd, NULL);
    DWORD curThread = GetCurrentThreadId();
    DWORD targetThread = GetWindowThreadProcessId(g_targetWindowHandle, NULL);

    AttachThreadInput(curThread, fgThread, TRUE);
    AttachThreadInput(curThread, targetThread, TRUE);

    SetWindowPos(g_targetWindowHandle, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW);
    BringWindowToTop(g_targetWindowHandle);
    SwitchToThisWindow(g_targetWindowHandle, TRUE);
    SetForegroundWindow(g_targetWindowHandle);

    AttachThreadInput(curThread, targetThread, FALSE);
    AttachThreadInput(curThread, fgThread, FALSE);

    Log(L"[DEBUG] Window activation complete");
}

// ============================================================================
// Toast Window
// ============================================================================

static bool IsPointInCloseButton(int x, int y) {
    int btnLeft = g_windowWidth - CLOSE_BUTTON_MARGIN - CLOSE_BUTTON_SIZE;
    int btnTop = CLOSE_BUTTON_MARGIN;
    return x >= btnLeft && x <= btnLeft + CLOSE_BUTTON_SIZE &&
           y >= btnTop && y <= btnTop + CLOSE_BUTTON_SIZE;
}

// Forward declarations for stacking functions
static bool IsBottomToast();
static void NotifyOtherToastsClosing();
static void AnimateToPosition();
static void RecalculatePosition();
static void NotifyAllToastsPauseTimer(bool pause);
static std::vector<struct ToastInfo> GetOtherToasts();

static LRESULT CALLBACK WndProc(HWND hWnd, UINT msg, WPARAM wParam, LPARAM lParam) {
    switch (msg) {
    case WM_PAINT: {
        PAINTSTRUCT ps;
        HDC hdc = BeginPaint(hWnd, &ps);

        // Dark background
        HBRUSH bgBrush = CreateSolidBrush(0x00333333);
        RECT rect = { 0, 0, g_windowWidth, g_windowHeight };
        FillRect(hdc, &rect, bgBrush);
        DeleteObject(bgBrush);

        // Border color: Yellow (0x00CFCF) for input mode, Orange (0x4B64B2) for normal
        COLORREF borderColor = g_inputMode ? 0x0000CFCF : 0x004B64B2;
        HBRUSH borderBrush = CreateSolidBrush(borderColor);
        RECT borders[] = {
            { 0, 0, g_windowWidth, 2 },
            { 0, g_windowHeight - 2, g_windowWidth, g_windowHeight },
            { 0, 0, 2, g_windowHeight },
            { g_windowWidth - 2, 0, g_windowWidth, g_windowHeight }
        };
        for (int i = 0; i < 4; i++) FillRect(hdc, &borders[i], borderBrush);
        DeleteObject(borderBrush);

        // Icon
        int iconX = g_iconPadding;
        int iconY = (g_windowHeight - g_iconSize) / 2;
        int textLeft = iconX + g_iconSize + g_iconPadding;

        if (g_appIcon) {
            DrawIconEx(hdc, iconX, iconY, g_appIcon, g_iconSize, g_iconSize, 0, NULL, DI_NORMAL);
        } else if (!g_defaultIconPath.empty()) {
            HICON hIcon = (HICON)LoadImageW(NULL, g_defaultIconPath.c_str(), IMAGE_ICON,
                g_iconSize, g_iconSize, LR_LOADFROMFILE);
            if (hIcon) {
                DrawIconEx(hdc, iconX, iconY, hIcon, g_iconSize, g_iconSize, 0, NULL, DI_NORMAL);
                DestroyIcon(hIcon);
            }
        }

        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, 0x00FFFFFF);

        // Title
        HFONT titleFont = CreateFontW(18, 0, 0, 0, FW_BOLD, 0, 0, 0, 0, 0, 0, 0, 0, g_fontFamily.c_str());
        HFONT oldFont = (HFONT)SelectObject(hdc, titleFont);
        RECT titleRect = { textLeft, 15, g_windowWidth - 10, 40 };
        DrawTextW(hdc, g_title.c_str(), -1, &titleRect, 0);
        SelectObject(hdc, oldFont);
        DeleteObject(titleFont);

        // Message
        HFONT msgFont = CreateFontW(14, 0, 0, 0, FW_NORMAL, 0, 0, 0, 0, 0, 0, 0, 0, g_fontFamily.c_str());
        oldFont = (HFONT)SelectObject(hdc, msgFont);
        SetTextColor(hdc, 0x00CCCCCC);
        RECT msgRect = { textLeft, 42, g_windowWidth - 10, g_windowHeight - 10 };
        DrawTextW(hdc, g_message.c_str(), -1, &msgRect, 0);
        SelectObject(hdc, oldFont);
        DeleteObject(msgFont);

        // Close button
        HFONT closeFont = CreateFontW(16, 0, 0, 0, FW_BOLD, 0, 0, 0, 0, 0, 0, 0, 0, L"Segoe UI");
        oldFont = (HFONT)SelectObject(hdc, closeFont);
        SetTextColor(hdc, 0x00888888);
        int btnLeft = g_windowWidth - CLOSE_BUTTON_MARGIN - CLOSE_BUTTON_SIZE;
        RECT closeRect = { btnLeft, CLOSE_BUTTON_MARGIN, btnLeft + CLOSE_BUTTON_SIZE, CLOSE_BUTTON_MARGIN + CLOSE_BUTTON_SIZE };
        DrawTextW(hdc, L"×", -1, &closeRect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
        SelectObject(hdc, oldFont);
        DeleteObject(closeFont);

        EndPaint(hWnd, &ps);
        return 0;
    }

    case WM_TIMER:
        if (wParam == TIMER_START_FADE) {
            KillTimer(hWnd, TIMER_START_FADE);
            g_isFading = true;
            SetTimer(hWnd, TIMER_FADE, 16, NULL);
        } else if (wParam == TIMER_FADE) {
            if (g_alpha > g_fadeStep) {
                g_alpha = (BYTE)(g_alpha - g_fadeStep);
                SetLayeredWindowAttributes(hWnd, 0, g_alpha, LWA_ALPHA);
            } else {
                g_isFading = false;
                KillTimer(hWnd, TIMER_FADE);
                NotifyOtherToastsClosing();  // Tell others to reposition
                DestroyWindow(hWnd);
            }
        } else if (wParam == TIMER_REPOSITION) {
            AnimateToPosition();
        } else if (wParam == TIMER_CHECK_BOTTOM) {
            // Check if we became the bottom toast
            if (IsBottomToast() && !g_isBottomToast) {
                g_isBottomToast = true;
                KillTimer(hWnd, TIMER_CHECK_BOTTOM);
                // Start fade timer now that we're at the bottom
                SetTimer(hWnd, TIMER_START_FADE, g_displayMs, NULL);
            }
        }
        return 0;

    case WM_TOAST_CHECK_POSITION: {
        // Another toast closed - wParam contains its Y position
        int closedToastY = (int)wParam;
        RECT myRect;
        GetWindowRect(g_hwnd, &myRect);

        // For bottom taskbar: if we're above the closed toast (lower Y), move down
        // For top taskbar: if we're below the closed toast (higher Y), move up
        if (g_taskbarEdge == ABE_TOP) {
            if (myRect.top > closedToastY) {
                g_targetY = myRect.top - g_windowHeight;
                SetTimer(g_hwnd, TIMER_REPOSITION, 16, NULL);
            }
        } else {
            if (myRect.top < closedToastY) {
                g_targetY = myRect.top + g_windowHeight;
                SetTimer(g_hwnd, TIMER_REPOSITION, 16, NULL);
            }
        }

        // Check if we became the bottom toast
        if (IsBottomToast() && !g_isBottomToast) {
            g_isBottomToast = true;
            KillTimer(hWnd, TIMER_CHECK_BOTTOM);
            SetTimer(hWnd, TIMER_START_FADE, g_displayMs, NULL);
        }
        return 0;
    }

    case WM_TOAST_PAUSE_TIMER: {
        // Another toast has mouse hover - pause/resume our timer
        bool pause = (wParam != 0);
        if (pause) {
            // Pause: stop fade timer and restore opacity
            if (g_isFading) {
                KillTimer(hWnd, TIMER_FADE);
                g_isFading = false;
                g_alpha = 230;
                SetLayeredWindowAttributes(hWnd, 0, g_alpha, LWA_ALPHA);
            }
            KillTimer(hWnd, TIMER_START_FADE);
        } else {
            // Resume: only bottom toast restarts timer
            if (g_isBottomToast && !g_mouseInside) {
                SetTimer(hWnd, TIMER_START_FADE, g_displayMs, NULL);
            }
        }
        return 0;
    }

    case WM_LBUTTONUP: {
        int x = LOWORD(lParam);
        int y = HIWORD(lParam);

        if (IsPointInCloseButton(x, y)) {
            Log(L"[DEBUG] Close button clicked");
            KillTimer(hWnd, TIMER_START_FADE);
            KillTimer(hWnd, TIMER_FADE);
            NotifyOtherToastsClosing();
            DestroyWindow(hWnd);
            return 0;
        }

        g_clicked = true;
        Log(L"[DEBUG] Toast clicked");
        KillTimer(hWnd, TIMER_START_FADE);
        KillTimer(hWnd, TIMER_FADE);
        NotifyOtherToastsClosing();
        ShowWindow(hWnd, SW_HIDE);
        ActivateWindow();
        DestroyWindow(hWnd);
        return 0;
    }

    case WM_RBUTTONUP:
        Log(L"[DEBUG] Right-click - closing");
        KillTimer(hWnd, TIMER_START_FADE);
        KillTimer(hWnd, TIMER_FADE);
        NotifyOtherToastsClosing();
        DestroyWindow(hWnd);
        return 0;

    case WM_MOUSEMOVE:
        if (!g_mouseInside) {
            g_mouseInside = true;

            TRACKMOUSEEVENT tme = { 0 };
            tme.cbSize = sizeof(tme);
            tme.dwFlags = TME_LEAVE;
            tme.hwndTrack = hWnd;
            TrackMouseEvent(&tme);

            // Pause all toasts (including self)
            NotifyAllToastsPauseTimer(true);
        }
        return 0;

    case WM_MOUSELEAVE:
        g_mouseInside = false;
        // Resume all toasts
        NotifyAllToastsPauseTimer(false);
        return 0;

    case WM_DESTROY:
        PostQuitMessage(0);
        return 0;
    }

    return DefWindowProcW(hWnd, msg, wParam, lParam);
}

// ============================================================================
// Toast Stacking - Telegram Style
// ============================================================================

struct ToastInfo {
    HWND hwnd;
    RECT rect;
};

static std::vector<ToastInfo> g_otherToasts;

static BOOL CALLBACK EnumToastWindows(HWND hwnd, LPARAM lParam) {
    // Skip our own window
    if (hwnd == g_hwnd) return TRUE;

    // Check if it's a toast window by class name
    wchar_t className[64];
    if (GetClassNameW(hwnd, className, 64) > 0) {
        if (wcscmp(className, TOAST_CLASS_NAME) == 0 && IsWindowVisible(hwnd)) {
            ToastInfo info;
            info.hwnd = hwnd;
            GetWindowRect(hwnd, &info.rect);
            g_otherToasts.push_back(info);
        }
    }
    return TRUE;
}

/// Get all other visible toast windows sorted by Y position
static std::vector<ToastInfo> GetOtherToasts() {
    g_otherToasts.clear();
    EnumWindows(EnumToastWindows, 0);

    // Sort by Y position (bottom to top for bottom taskbar, top to bottom for top taskbar)
    if (g_taskbarEdge == ABE_TOP) {
        std::sort(g_otherToasts.begin(), g_otherToasts.end(),
            [](const ToastInfo& a, const ToastInfo& b) { return a.rect.top < b.rect.top; });
    } else {
        std::sort(g_otherToasts.begin(), g_otherToasts.end(),
            [](const ToastInfo& a, const ToastInfo& b) { return a.rect.bottom > b.rect.bottom; });
    }

    return g_otherToasts;
}

/// Calculate base position (where bottom toast should be)
static int GetBaseY() {
    if (g_taskbarEdge == ABE_TOP) {
        return g_workArea.top;
    } else {
        return g_workArea.bottom - g_windowHeight;
    }
}

/// Calculate Y position for stacking (new toast goes on top of existing ones)
static int CalculateStackedY() {
    auto toasts = GetOtherToasts();

    if (toasts.empty()) {
        return GetBaseY();
    }

    // Stack above/below existing toasts (no gap)
    if (g_taskbarEdge == ABE_TOP) {
        // For top taskbar, stack downward
        int lowestBottom = g_workArea.top;
        for (const auto& t : toasts) {
            if (t.rect.bottom > lowestBottom) {
                lowestBottom = t.rect.bottom;
            }
        }
        return lowestBottom;  // New toast below existing
    } else {
        // For bottom taskbar, stack upward
        int highestTop = g_workArea.bottom;
        for (const auto& t : toasts) {
            if (t.rect.top < highestTop) {
                highestTop = t.rect.top;
            }
        }
        return highestTop - g_windowHeight;  // New toast above existing
    }
}

/// Check if this toast is the bottommost one
static bool IsBottomToast() {
    auto toasts = GetOtherToasts();

    if (toasts.empty()) return true;

    RECT myRect;
    GetWindowRect(g_hwnd, &myRect);

    if (g_taskbarEdge == ABE_TOP) {
        // For top taskbar, bottom = highest Y (top value)
        for (const auto& t : toasts) {
            if (t.rect.top < myRect.top) {
                return false;  // Another toast is above us (closer to taskbar)
            }
        }
    } else {
        // For bottom taskbar, bottom = highest bottom value
        for (const auto& t : toasts) {
            if (t.rect.bottom > myRect.bottom) {
                return false;  // Another toast is below us (closer to taskbar)
            }
        }
    }

    return true;
}

/// Notify all other toasts that we're closing, pass our Y position
static void NotifyOtherToastsClosing() {
    RECT myRect;
    GetWindowRect(g_hwnd, &myRect);

    auto toasts = GetOtherToasts();
    for (const auto& t : toasts) {
        // Send our Y position so they know where the gap is
        PostMessage(t.hwnd, WM_TOAST_CHECK_POSITION, (WPARAM)myRect.top, 0);
    }
}

/// Notify all toasts (including self) to pause or resume timer
static void NotifyAllToastsPauseTimer(bool pause) {
    // Send to self
    PostMessage(g_hwnd, WM_TOAST_PAUSE_TIMER, pause ? 1 : 0, 0);

    // Send to others
    auto toasts = GetOtherToasts();
    for (const auto& t : toasts) {
        PostMessage(t.hwnd, WM_TOAST_PAUSE_TIMER, pause ? 1 : 0, 0);
    }
}

/// Smoothly animate to target Y position
static void AnimateToPosition() {
    RECT rect;
    GetWindowRect(g_hwnd, &rect);
    int currentY = rect.top;

    if (currentY == g_targetY) {
        KillTimer(g_hwnd, TIMER_REPOSITION);
        return;
    }

    // Move 40% of remaining distance each frame (faster animation)
    int diff = g_targetY - currentY;
    int step = diff * 2 / 5;
    if (step == 0) step = (diff > 0) ? 2 : -2;

    int newY = currentY + step;

    // Snap when close enough
    if (abs(g_targetY - newY) < 4) {
        newY = g_targetY;
    }

    SetWindowPos(g_hwnd, NULL, rect.left, newY, 0, 0,
        SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);

    if (newY == g_targetY) {
        KillTimer(g_hwnd, TIMER_REPOSITION);
    }
}

/// Recalculate position based on other toasts
/// Uses HWND value for ordering (lower HWND = created earlier = closer to taskbar)
static void RecalculatePosition() {
    auto toasts = GetOtherToasts();

    // Count how many toasts have lower HWND than us (they should be below/above us near taskbar)
    int toastsBeforeUs = 0;
    for (const auto& t : toasts) {
        if ((intptr_t)t.hwnd < (intptr_t)g_hwnd) {
            toastsBeforeUs++;
        }
    }

    if (g_taskbarEdge == ABE_TOP) {
        g_targetY = g_workArea.top + toastsBeforeUs * g_windowHeight;
    } else {
        g_targetY = g_workArea.bottom - g_windowHeight - toastsBeforeUs * g_windowHeight;
    }

    RECT myRect;
    GetWindowRect(g_hwnd, &myRect);
    if (myRect.top != g_targetY) {
        SetTimer(g_hwnd, TIMER_REPOSITION, 16, NULL);
    }
}

static void ShowToast() {
    // Use fixed class name for stacking

    // Load custom font
    if (!g_fontFile.empty() && GetFileAttributesW(g_fontFile.c_str()) != INVALID_FILE_ATTRIBUTES) {
        if (AddFontResourceExW(g_fontFile.c_str(), FR_PRIVATE, NULL) > 0) {
            g_fontLoaded = true;
            std::wstring fileName = g_fontFile;
            size_t pos = fileName.find_last_of(L"\\/");
            if (pos != std::wstring::npos) fileName = fileName.substr(pos + 1);
            pos = fileName.find_last_of(L'.');
            if (pos != std::wstring::npos) fileName = fileName.substr(0, pos);

            for (const wchar_t* suffix : {L"-Regular", L"-Bold", L"-Italic", L"-Light", L"-Medium"}) {
                size_t spos = fileName.find(suffix);
                if (spos != std::wstring::npos) { fileName = fileName.substr(0, spos); break; }
            }

            std::wstring result;
            for (size_t i = 0; i < fileName.size(); i++) {
                if (i > 0 && iswupper(fileName[i]) && !iswupper(fileName[i-1])) result += L' ';
                result += fileName[i];
            }
            g_fontFamily = result;
        }
    }

    // Load icon from saved path
    if (!g_savedIconPath.empty()) {
        HICON hIconLarge = NULL, hIconSmall = NULL;
        ExtractIconExW(g_savedIconPath.c_str(), 0, &hIconLarge, &hIconSmall, 1);
        if (hIconSmall) DestroyIcon(hIconSmall);
        g_appIcon = hIconLarge;
    }

    // Fade step calculation
    int fadeTicks = g_fadeMs / 16;
    if (fadeTicks < 1) fadeTicks = 1;
    g_fadeStep = (230 / fadeTicks) + 1;

    // Register window class (use fixed class name for stacking)
    g_className = TOAST_CLASS_NAME;
    WNDCLASSEXW wc = { 0 };
    wc.cbSize = sizeof(wc);
    wc.lpfnWndProc = WndProc;
    wc.hInstance = GetModuleHandleW(NULL);
    wc.hCursor = LoadCursorW(NULL, IDC_HAND);
    wc.lpszClassName = TOAST_CLASS_NAME;
    RegisterClassExW(&wc);  // OK if already registered

    // Position near cursor's monitor
    POINT cursorPos;
    GetCursorPos(&cursorPos);
    HMONITOR hMonitor = MonitorFromPoint(cursorPos, MONITOR_DEFAULTTOPRIMARY);

    MONITORINFO mi = { 0 };
    mi.cbSize = sizeof(mi);
    GetMonitorInfoW(hMonitor, &mi);
    g_workArea = mi.rcWork;  // Cache for stacking calculations

    // Taskbar detection
    APPBARDATA abd = { 0 };
    abd.cbSize = sizeof(abd);
    g_taskbarEdge = ABE_BOTTOM;
    if (SHAppBarMessage(ABM_GETTASKBARPOS, &abd)) {
        g_taskbarEdge = abd.uEdge;
    }

    // Calculate stacked position
    int x = g_workArea.right - g_windowWidth;
    if (g_taskbarEdge == ABE_LEFT) {
        x = g_workArea.left;
    }
    int y = CalculateStackedY();
    g_targetY = y;

    // Create window
    g_hwnd = CreateWindowExW(
        WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_NOACTIVATE,
        g_className.c_str(), L"Toast",
        WS_POPUP,
        x, y, g_windowWidth, g_windowHeight,
        NULL, NULL, GetModuleHandleW(NULL), NULL
    );

    if (!g_hwnd) return;

    SetLayeredWindowAttributes(g_hwnd, 0, g_alpha, LWA_ALPHA);

    // Sound
    if (!g_soundFile.empty() && GetFileAttributesW(g_soundFile.c_str()) != INVALID_FILE_ATTRIBUTES) {
        PlaySoundW(g_soundFile.c_str(), NULL, SND_FILENAME | SND_ASYNC);
    } else {
        MessageBeep(MB_ICONASTERISK);
    }

    // Check if we're the bottom toast - only bottom toast starts fade timer
    g_isBottomToast = IsBottomToast();
    if (g_isBottomToast) {
        SetTimer(g_hwnd, TIMER_START_FADE, g_displayMs, NULL);
    } else {
        // Periodically check if we became the bottom toast
        SetTimer(g_hwnd, TIMER_CHECK_BOTTOM, 200, NULL);
    }

    ShowWindow(g_hwnd, SW_SHOWNOACTIVATE);
    UpdateWindow(g_hwnd);

    // Message loop
    MSG msg;
    while (GetMessageW(&msg, NULL, 0, 0) > 0) {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }

    // Cleanup
    UnregisterClassW(g_className.c_str(), GetModuleHandleW(NULL));
    if (g_appIcon) DestroyIcon(g_appIcon);
    if (g_pAutomation) { g_pAutomation->Release(); g_pAutomation = NULL; }
    if (g_fontLoaded && !g_fontFile.empty()) {
        RemoveFontResourceExW(g_fontFile.c_str(), FR_PRIVATE, NULL);
    }
}

// ============================================================================
// Asset Discovery
// ============================================================================

static std::wstring FindFirstFile(const std::wstring& dir, const std::wstring& pattern) {
    WIN32_FIND_DATAW fd;
    HANDLE hFind = FindFirstFileW((dir + L"\\" + pattern).c_str(), &fd);
    if (hFind == INVALID_HANDLE_VALUE) return L"";
    std::wstring result = dir + L"\\" + fd.cFileName;
    FindClose(hFind);
    return result;
}

// ============================================================================
// Entry Point
// ============================================================================

int WINAPI WinMain(HINSTANCE, HINSTANCE, LPSTR, int) {
    // CRITICAL: Capture foreground window IMMEDIATELY before any other operation
    // This prevents race condition where user switches away during stdin/COM init
    g_immediateHwnd = GetForegroundWindow();

    SetConsoleOutputCP(CP_UTF8);
    SetConsoleCP(CP_UTF8);
    CoInitializeEx(NULL, COINIT_MULTITHREADED);

    int argc;
    LPWSTR* argv = CommandLineToArgvW(GetCommandLineW(), &argc);
    if (!argv) {
        CoUninitialize();
        return 1;
    }

    // Parse arguments
    bool saveMode = false;
    bool notifyMode = false;
    bool inputMode = false;       // Notification hook (input required)
    bool notifyShowMode = false;  // Internal: actually show the toast

    for (int i = 1; i < argc; i++) {
        std::wstring arg = argv[i];

        if (arg == L"--save") {
            saveMode = true;
        } else if (arg == L"--notify") {
            notifyMode = true;
        } else if (arg == L"--input") {
            inputMode = true;
        } else if (arg == L"--notify-show") {
            notifyShowMode = true;  // Internal flag for spawned process
        } else if (arg == L"--input-mode") {
            g_inputMode = true;     // Internal flag for input style
        } else if (arg == L"--debug" || arg == L"-d") {
            g_debug = true;
            wchar_t exePath[MAX_PATH];
            GetModuleFileNameW(NULL, exePath, MAX_PATH);
            std::wstring exeDir = exePath;
            size_t pos = exeDir.find_last_of(L"\\/");
            if (pos != std::wstring::npos) exeDir = exeDir.substr(0, pos);
            g_logFile = exeDir + L"\\debug.log";

            FILE* f = _wfopen(g_logFile.c_str(), L"w, ccs=UTF-8");
            if (f) {
                fwprintf(f, L"=== ToastWindow Debug Log ===\n");
                fclose(f);
            }
        } else if (arg == L"--session" && i + 1 < argc) {
            g_sessionId = argv[++i];
        }
    }

    LocalFree(argv);

    if (saveMode) {
        SaveState();
        CoUninitialize();
        return 0;
    }

    if (notifyMode) {
        // Read JSON from stdin to get session_id
        std::string jsonInput = ReadStdinJson();
        g_sessionId = ExtractJsonStringValue(jsonInput, "session_id");

        if (g_sessionId.empty()) {
            Log(L"[DEBUG] No session_id in stdin JSON, cannot show notification");
            CoUninitialize();
            return 1;
        }

        Log(L"[DEBUG] Notify mode, session_id: %ls", g_sessionId.c_str());

        // Build command line for child process - only pass session ID
        wchar_t exePath[MAX_PATH];
        GetModuleFileNameW(NULL, exePath, MAX_PATH);

        std::wstring cmdLine = L"\"";
        cmdLine += exePath;
        cmdLine += L"\" --notify-show --session \"";
        cmdLine += g_sessionId;
        cmdLine += L"\"";
        if (g_debug) {
            cmdLine += L" --debug";
        }

        // Create detached process
        STARTUPINFOW si = { sizeof(si) };
        PROCESS_INFORMATION pi;
        si.dwFlags = STARTF_USESHOWWINDOW;
        si.wShowWindow = SW_HIDE;

        if (CreateProcessW(NULL, (LPWSTR)cmdLine.c_str(), NULL, NULL, FALSE,
                          CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS,
                          NULL, NULL, &si, &pi)) {
            CloseHandle(pi.hProcess);
            CloseHandle(pi.hThread);
        }

        CoUninitialize();
        return 0;  // Exit immediately
    }

    if (inputMode) {
        // Read JSON from stdin to get session_id (same as notifyMode but with input style)
        std::string jsonInput = ReadStdinJson();
        g_sessionId = ExtractJsonStringValue(jsonInput, "session_id");

        if (g_sessionId.empty()) {
            Log(L"[DEBUG] No session_id in stdin JSON, cannot show notification");
            CoUninitialize();
            return 1;
        }

        Log(L"[DEBUG] Input mode, session_id: %ls", g_sessionId.c_str());

        // Build command line for child process - pass session ID and input-mode flag
        wchar_t exePath[MAX_PATH];
        GetModuleFileNameW(NULL, exePath, MAX_PATH);

        std::wstring cmdLine = L"\"";
        cmdLine += exePath;
        cmdLine += L"\" --notify-show --input-mode --session \"";
        cmdLine += g_sessionId;
        cmdLine += L"\"";
        if (g_debug) {
            cmdLine += L" --debug";
        }

        // Create detached process
        STARTUPINFOW si = { sizeof(si) };
        PROCESS_INFORMATION pi;
        si.dwFlags = STARTF_USESHOWWINDOW;
        si.wShowWindow = SW_HIDE;

        if (CreateProcessW(NULL, (LPWSTR)cmdLine.c_str(), NULL, NULL, FALSE,
                          CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS,
                          NULL, NULL, &si, &pi)) {
            CloseHandle(pi.hProcess);
            CloseHandle(pi.hThread);
        }

        CoUninitialize();
        return 0;  // Exit immediately
    }

    if (notifyShowMode) {
        // Session ID was already parsed above
        if (g_sessionId.empty()) {
            Log(L"[DEBUG] No session ID provided");
            CoUninitialize();
            return 1;
        }

        // Load all state from file
        LoadState(g_sessionId);

        // Use user prompt as message (truncated if needed)
        if (g_inputMode) {
            // Input required mode - different title and default message
            g_title = L"Input Required";
            if (g_userPrompt.empty()) {
                g_message = L"Claude needs your input";
            } else {
                g_message = g_userPrompt;
            }
        } else {
            // Normal completion mode
            g_title = L"Claude Code";
            if (g_userPrompt.empty()) {
                g_message = L"Task completed";
            } else {
                g_message = g_userPrompt;
            }
        }

        // Replace newlines with spaces for single-line display
        for (size_t i = 0; i < g_message.length(); i++) {
            if (g_message[i] == L'\n' || g_message[i] == L'\r') {
                g_message[i] = L' ';
            }
        }
        // Truncate with ellipsis if too long
        const size_t maxLen = 35;
        if (g_message.length() > maxLen) {
            g_message = g_message.substr(0, maxLen) + L"...";
        }

        // Set up asset paths
        wchar_t exePath[MAX_PATH];
        GetModuleFileNameW(NULL, exePath, MAX_PATH);
        std::wstring exeDir = exePath;
        size_t pos = exeDir.find_last_of(L"\\/");
        if (pos != std::wstring::npos) exeDir = exeDir.substr(0, pos);

        g_soundFile = FindFirstFile(exeDir + L"\\assets\\sound", L"*.wav");
        g_fontFile = FindFirstFile(exeDir + L"\\assets\\fonts", L"*.ttf");
        if (g_fontFile.empty()) {
            g_fontFile = FindFirstFile(exeDir + L"\\assets\\fonts", L"*.otf");
        }
        g_defaultIconPath = FindFirstFile(exeDir + L"\\assets\\img", L"*.ico");

        ShowToast();

        // Delete state file after use (cleanup)
        std::wstring stateFile = GetStateFilePath(g_sessionId);
        DeleteFileW(stateFile.c_str());

        CoUninitialize();
        return 0;
    }

    // No valid mode specified
    printf("Usage:\n");
    printf("  ToastWindow.exe --save      Save window state (UserPromptSubmit hook)\n");
    printf("  ToastWindow.exe --notify    Show notification (Stop hook)\n");
    printf("  ToastWindow.exe --input     Show input-required notification (Notification hook)\n");
    printf("\n");
    printf("Both modes read session_id from stdin JSON for state file isolation.\n");

    CoUninitialize();
    return 1;
}

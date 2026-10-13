#include <flutter/dart_project.h>
#include <flutter/flutter_view_controller.h>
#include <windows.h>
#include <werapi.h>

#include "flutter_window.h"
#include "utils.h"

namespace {

constexpr wchar_t kWindowTitle[] = L"AryaVault";
// Per logon session, so two copies of the app cannot open the same vault (SEC-S06 spirit: one
// writer). The Flutter runner's window class name is fixed by win32_window.cpp.
constexpr wchar_t kSingleInstanceMutex[] = L"Local\\AryaVault.SingleInstance.v1";
constexpr wchar_t kRunnerWindowClass[] = L"FLUTTER_RUNNER_WIN32_WINDOW";

// A crash must not write the process memory (which can hold an unlocked vault) into a Windows
// Error Reporting report: no dialog, and no heap in any report that is made. This cannot stop a
// machine-wide WER LocalDumps policy; locking zeroes the keys, so the exposure is the unlocked
// time only (documented in docs/reviews/m2-windows-evidence.md).
void HardenCrashReporting() {
  ::SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX |
                 SEM_NOOPENFILEERRORBOX);
  ::WerSetFlags(WER_FAULT_REPORTING_FLAG_NOHEAP |
                WER_FAULT_REPORTING_NO_UI);
}

// Brings the running copy to the front. Best effort: if it cannot be found, just exit.
void ActivateRunningInstance() {
  HWND other = ::FindWindowW(kRunnerWindowClass, kWindowTitle);
  if (other == nullptr) {
    return;
  }
  if (::IsIconic(other)) {
    ::ShowWindow(other, SW_RESTORE);
  }
  ::SetForegroundWindow(other);
}

}  // namespace

int APIENTRY wWinMain(_In_ HINSTANCE instance, _In_opt_ HINSTANCE prev,
                      _In_ wchar_t *command_line, _In_ int show_command) {
  HardenCrashReporting();

  // One process per user session. The handle stays open until the process ends.
  HANDLE single_instance = ::CreateMutexW(nullptr, TRUE, kSingleInstanceMutex);
  if (single_instance != nullptr && ::GetLastError() == ERROR_ALREADY_EXISTS) {
    ActivateRunningInstance();
    ::CloseHandle(single_instance);
    return EXIT_SUCCESS;
  }

  // Attach to console when present (e.g., 'flutter run') or create a
  // new console when running with a debugger.
  if (!::AttachConsole(ATTACH_PARENT_PROCESS) && ::IsDebuggerPresent()) {
    CreateAndAttachConsole();
  }

  // Initialize COM, so that it is available for use in the library and/or
  // plugins.
  ::CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);

  flutter::DartProject project(L"data");

  std::vector<std::string> command_line_arguments =
      GetCommandLineArguments();

  project.set_dart_entrypoint_arguments(std::move(command_line_arguments));

  FlutterWindow window(project);
  Win32Window::Point origin(10, 10);
  Win32Window::Size size(1280, 720);
  if (!window.Create(kWindowTitle, origin, size)) {
    return EXIT_FAILURE;
  }
  window.SetQuitOnClose(true);

  ::MSG msg;
  while (::GetMessage(&msg, nullptr, 0, 0)) {
    ::TranslateMessage(&msg);
    ::DispatchMessage(&msg);
  }

  ::CoUninitialize();
  return EXIT_SUCCESS;
}

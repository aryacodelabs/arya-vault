#include "platform_channel.h"

#include <shobjidl.h>
#include <wtsapi32.h>

#include <flutter/standard_method_codec.h>

#include <algorithm>
#include <cstdint>
#include <string>
#include <vector>

namespace {

constexpr char kChannelName[] = "aryavault/platform";

// Clipboard formats that keep a value out of Win+V history, cloud sync and clipboard
// monitors (docs/08 SEC-H01; checked by spike S5, docs/reviews/m2-spikes.md).
constexpr wchar_t kExcludeMonitor[] =
    L"ExcludeClipboardContentFromMonitorProcessing";
constexpr wchar_t kCanIncludeInHistory[] = L"CanIncludeClipboardContentInHistory";
constexpr wchar_t kCanUploadToCloud[] = L"CanUploadToCloudClipboard";

// Another process can hold the clipboard for a moment.
bool OpenClipboardWithRetry(HWND window) {
  for (int i = 0; i < 10; ++i) {
    if (OpenClipboard(window)) {
      return true;
    }
    Sleep(20);
  }
  return false;
}

// Puts `size` bytes on the clipboard under `format`. Takes ownership of the memory on success.
bool SetBytes(UINT format, const void* data, size_t size) {
  HGLOBAL mem = GlobalAlloc(GMEM_MOVEABLE, size);
  if (mem == nullptr) {
    return false;
  }
  void* dst = GlobalLock(mem);
  if (dst == nullptr) {
    GlobalFree(mem);
    return false;
  }
  memcpy(dst, data, size);
  GlobalUnlock(mem);
  if (SetClipboardData(format, mem) == nullptr) {
    GlobalFree(mem);
    return false;
  }
  return true;
}

enum class Choice { kChosen, kCancelled, kFailed };

// The system file dialog (save or open). Nothing but the chosen path leaves it. The filter
// shows the patterns themselves (for example "*.csv"), so the native layer has no
// user-visible text of its own to translate.
Choice ChooseFile(HWND owner, bool save, const std::wstring& suggested_name,
                  const std::vector<std::wstring>& extensions,
                  std::wstring* path) {
  IFileDialog* dialog = nullptr;
  HRESULT hr = CoCreateInstance(
      save ? CLSID_FileSaveDialog : CLSID_FileOpenDialog, nullptr,
      CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&dialog));
  if (FAILED(hr)) {
    return Choice::kFailed;
  }
  Choice outcome = Choice::kFailed;
  DWORD options = 0;
  if (SUCCEEDED(dialog->GetOptions(&options))) {
    options |= FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST |
               (save ? FOS_OVERWRITEPROMPT : FOS_FILEMUSTEXIST);
    dialog->SetOptions(options);
  }
  std::wstring pattern;
  for (const std::wstring& ext : extensions) {
    if (!pattern.empty()) {
      pattern += L";";
    }
    pattern += L"*." + ext;
  }
  COMDLG_FILTERSPEC spec = {pattern.c_str(), pattern.c_str()};
  if (!pattern.empty()) {
    dialog->SetFileTypes(1, &spec);
    dialog->SetFileTypeIndex(1);
    if (save) {
      dialog->SetDefaultExtension(extensions.front().c_str());
    }
  }
  if (save && !suggested_name.empty()) {
    dialog->SetFileName(suggested_name.c_str());
  }
  hr = dialog->Show(owner);
  if (hr == HRESULT_FROM_WIN32(ERROR_CANCELLED)) {
    outcome = Choice::kCancelled;
  } else if (SUCCEEDED(hr)) {
    IShellItem* item = nullptr;
    if (SUCCEEDED(dialog->GetResult(&item))) {
      PWSTR name = nullptr;
      if (SUCCEEDED(item->GetDisplayName(SIGDN_FILESYSPATH, &name))) {
        *path = name;
        CoTaskMemFree(name);
        outcome = Choice::kChosen;
      }
      item->Release();
    }
  }
  dialog->Release();
  return outcome;
}

std::wstring ToWide(const std::string& utf8) {
  if (utf8.empty()) {
    return std::wstring();
  }
  const int n = MultiByteToWideChar(CP_UTF8, 0, utf8.data(),
                                    static_cast<int>(utf8.size()), nullptr, 0);
  if (n <= 0) {
    return std::wstring();
  }
  std::wstring out(static_cast<size_t>(n), L'\0');
  MultiByteToWideChar(CP_UTF8, 0, utf8.data(), static_cast<int>(utf8.size()),
                      out.data(), n);
  return out;
}

// Largest file the picker hands to Dart (import files; docs/05 limits are far lower).
constexpr LONGLONG kMaxPickBytes = 64LL * 1024 * 1024;

}  // namespace

PlatformChannel::PlatformChannel(flutter::BinaryMessenger* messenger,
                                 HWND window)
    : window_(window) {
  channel_ = std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
      messenger, kChannelName, &flutter::StandardMethodCodec::GetInstance());
  channel_->SetMethodCallHandler(
      [this](const Call& call, std::unique_ptr<Result> result) {
        Handle(call, std::move(result));
      });
  // Lock/unlock of the workstation arrives as WM_WTSSESSION_CHANGE.
  session_registered_ =
      WTSRegisterSessionNotification(window_, NOTIFY_FOR_THIS_SESSION) != FALSE;
}

PlatformChannel::~PlatformChannel() {
  if (session_registered_) {
    WTSUnRegisterSessionNotification(window_);
  }
  channel_->SetMethodCallHandler(nullptr);
}

void PlatformChannel::Handle(const Call& call, std::unique_ptr<Result> result) {
  const std::string& name = call.method_name();
  if (name == "clipboard.write") {
    WriteClipboard(call, std::move(result));
  } else if (name == "clipboard.fingerprint") {
    Fingerprint(std::move(result));
  } else if (name == "clipboard.clear") {
    ClearClipboard(std::move(result));
  } else if (name == "capture.setBlocked") {
    SetCaptureBlocked(call, std::move(result));
  } else if (name == "file.save") {
    SaveFile(call, std::move(result));
  } else if (name == "file.pick") {
    PickFile(call, std::move(result));
  } else {
    result->NotImplemented();
  }
}

void PlatformChannel::WriteClipboard(const Call& call,
                                     std::unique_ptr<Result> result) {
  const auto* args = std::get_if<flutter::EncodableMap>(call.arguments());
  if (args == nullptr) {
    result->Error("bad_args", "expected a map");
    return;
  }
  auto bytes_it = args->find(flutter::EncodableValue("bytes"));
  auto sensitive_it = args->find(flutter::EncodableValue("sensitive"));
  if (bytes_it == args->end() || sensitive_it == args->end()) {
    result->Error("bad_args", "bytes and sensitive are required");
    return;
  }
  const auto* utf8 = std::get_if<std::vector<uint8_t>>(&bytes_it->second);
  const auto* sensitive = std::get_if<bool>(&sensitive_it->second);
  if (utf8 == nullptr || sensitive == nullptr) {
    result->Error("bad_args", "wrong argument types");
    return;
  }

  // UTF-8 -> UTF-16 (the clipboard's text format), then wipe our copy after the write.
  std::vector<wchar_t> wide;
  if (!utf8->empty()) {
    const int needed = MultiByteToWideChar(
        CP_UTF8, MB_ERR_INVALID_CHARS, reinterpret_cast<const char*>(utf8->data()),
        static_cast<int>(utf8->size()), nullptr, 0);
    if (needed <= 0) {
      result->Error("bad_text", "the text is not valid UTF-8");
      return;
    }
    wide.resize(static_cast<size_t>(needed));
    MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS,
                        reinterpret_cast<const char*>(utf8->data()),
                        static_cast<int>(utf8->size()), wide.data(), needed);
  }
  wide.push_back(L'\0');

  bool ok = false;
  if (OpenClipboardWithRetry(window_)) {
    EmptyClipboard();
    ok = SetBytes(CF_UNICODETEXT, wide.data(), wide.size() * sizeof(wchar_t));
    if (ok && *sensitive) {
      const UINT exclude = RegisterClipboardFormatW(kExcludeMonitor);
      const UINT history = RegisterClipboardFormatW(kCanIncludeInHistory);
      const UINT cloud = RegisterClipboardFormatW(kCanUploadToCloud);
      const DWORD zero = 0;
      // Failing to set a hygiene format is a failure: never leave a secret in history.
      ok = exclude != 0 && history != 0 && cloud != 0 &&
           SetBytes(exclude, &zero, sizeof(zero)) &&
           SetBytes(history, &zero, sizeof(zero)) &&
           SetBytes(cloud, &zero, sizeof(zero));
      if (!ok) {
        EmptyClipboard();
      }
    }
    CloseClipboard();
  }
  SecureZeroMemory(wide.data(), wide.size() * sizeof(wchar_t));

  if (ok) {
    result->Success();
  } else {
    result->Error("clipboard_failed", "could not write the clipboard");
  }
}

// An opaque value that changes whenever anything writes the clipboard. It carries no
// content. Read after CloseClipboard (a single copy bumps the number several times).
void PlatformChannel::Fingerprint(std::unique_ptr<Result> result) {
  if (!IsClipboardFormatAvailable(CF_UNICODETEXT)) {
    result->Success(flutter::EncodableValue());
    return;
  }
  const DWORD seq = GetClipboardSequenceNumber();
  result->Success(flutter::EncodableValue("seq:" + std::to_string(seq)));
}

void PlatformChannel::ClearClipboard(std::unique_ptr<Result> result) {
  if (!OpenClipboardWithRetry(window_)) {
    result->Error("clipboard_failed", "could not open the clipboard");
    return;
  }
  const BOOL ok = EmptyClipboard();
  CloseClipboard();
  if (ok) {
    result->Success();
  } else {
    result->Error("clipboard_failed", "could not clear the clipboard");
  }
}

void PlatformChannel::SetCaptureBlocked(const Call& call,
                                        std::unique_ptr<Result> result) {
  const auto* blocked = std::get_if<bool>(call.arguments());
  if (blocked == nullptr) {
    result->Error("bad_args", "expected a bool");
    return;
  }
  const DWORD affinity = *blocked ? WDA_EXCLUDEFROMCAPTURE : WDA_NONE;
  if (SetWindowDisplayAffinity(window_, affinity)) {
    result->Success();
  } else {
    result->Error("capture_failed", "could not change capture protection");
  }
}

// Asks where to save, then writes the bytes there. Returns false if the user cancelled.
void PlatformChannel::SaveFile(const Call& call,
                               std::unique_ptr<Result> result) {
  const auto* args = std::get_if<flutter::EncodableMap>(call.arguments());
  if (args == nullptr) {
    result->Error("bad_args", "expected a map");
    return;
  }
  auto name_it = args->find(flutter::EncodableValue("name"));
  auto bytes_it = args->find(flutter::EncodableValue("bytes"));
  if (name_it == args->end() || bytes_it == args->end()) {
    result->Error("bad_args", "name and bytes are required");
    return;
  }
  const auto* name = std::get_if<std::string>(&name_it->second);
  const auto* bytes = std::get_if<std::vector<uint8_t>>(&bytes_it->second);
  if (name == nullptr || bytes == nullptr) {
    result->Error("bad_args", "wrong argument types");
    return;
  }
  const std::wstring suggested = ToWide(*name);
  std::vector<std::wstring> extensions;
  const size_t dot = suggested.find_last_of(L'.');
  if (dot != std::wstring::npos && dot + 1 < suggested.size()) {
    extensions.push_back(suggested.substr(dot + 1));
  }
  std::wstring path;
  const Choice choice = ChooseFile(window_, true, suggested, extensions, &path);
  if (choice == Choice::kCancelled) {
    result->Success(flutter::EncodableValue(false));
    return;
  }
  if (choice == Choice::kFailed) {
    result->Error("dialog_failed", "could not show the save dialog");
    return;
  }
  HANDLE file = CreateFileW(path.c_str(), GENERIC_WRITE, 0, nullptr,
                            CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
  if (file == INVALID_HANDLE_VALUE) {
    result->Error("save_failed", "could not create the file");
    return;
  }
  bool ok = true;
  size_t written_total = 0;
  while (written_total < bytes->size()) {
    DWORD written = 0;
    const DWORD chunk = static_cast<DWORD>(
        std::min<size_t>(bytes->size() - written_total, 1u << 20));
    if (!WriteFile(file, bytes->data() + written_total, chunk, &written,
                   nullptr) ||
        written == 0) {
      ok = false;
      break;
    }
    written_total += written;
  }
  ok = CloseHandle(file) && ok;
  if (ok) {
    result->Success(flutter::EncodableValue(true));
  } else {
    result->Error("save_failed", "could not write the file");
  }
}

// Asks for a file and returns its bytes, or null if the user cancelled.
void PlatformChannel::PickFile(const Call& call,
                               std::unique_ptr<Result> result) {
  std::vector<std::wstring> extensions;
  if (const auto* args = std::get_if<flutter::EncodableList>(call.arguments())) {
    for (const flutter::EncodableValue& v : *args) {
      if (const auto* s = std::get_if<std::string>(&v)) {
        extensions.push_back(ToWide(*s));
      }
    }
  }
  std::wstring path;
  const Choice choice = ChooseFile(window_, false, L"", extensions, &path);
  if (choice == Choice::kCancelled) {
    result->Success(flutter::EncodableValue());
    return;
  }
  if (choice == Choice::kFailed) {
    result->Error("dialog_failed", "could not show the open dialog");
    return;
  }
  HANDLE file =
      CreateFileW(path.c_str(), GENERIC_READ, FILE_SHARE_READ, nullptr,
                  OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
  if (file == INVALID_HANDLE_VALUE) {
    result->Error("read_failed", "could not open the file");
    return;
  }
  LARGE_INTEGER size = {};
  if (!GetFileSizeEx(file, &size)) {
    CloseHandle(file);
    result->Error("read_failed", "could not read the file size");
    return;
  }
  if (size.QuadPart > kMaxPickBytes) {
    CloseHandle(file);
    result->Error("too_large", "the file is too large");
    return;
  }
  std::vector<uint8_t> data(static_cast<size_t>(size.QuadPart));
  size_t done = 0;
  bool ok = true;
  while (done < data.size()) {
    DWORD read = 0;
    const DWORD chunk =
        static_cast<DWORD>(std::min<size_t>(data.size() - done, 1u << 20));
    if (!ReadFile(file, data.data() + done, chunk, &read, nullptr) ||
        read == 0) {
      ok = false;
      break;
    }
    done += read;
  }
  CloseHandle(file);
  if (!ok) {
    SecureZeroMemory(data.data(), data.size());
    result->Error("read_failed", "could not read the file");
    return;
  }
  result->Success(flutter::EncodableValue(std::move(data)));
}

void PlatformChannel::Emit(const char* event) {
  channel_->InvokeMethod(
      "event", std::make_unique<flutter::EncodableValue>(std::string(event)));
}

void PlatformChannel::OnWindowMessage(UINT message, WPARAM wparam,
                                      LPARAM /*lparam*/) {
  switch (message) {
    case WM_POWERBROADCAST:
      if (wparam == PBT_APMSUSPEND) {
        Emit("sleep");
      } else if (wparam == PBT_APMRESUMEAUTOMATIC ||
                 wparam == PBT_APMRESUMESUSPEND) {
        Emit("resume");
      }
      break;
    case WM_WTSSESSION_CHANGE:
      if (wparam == WTS_SESSION_LOCK) {
        Emit("screenLocked");
      } else if (wparam == WTS_SESSION_UNLOCK) {
        Emit("screenUnlocked");
      }
      break;
    case WM_SIZE:
      if (wparam == SIZE_MINIMIZED) {
        if (!minimized_) {
          minimized_ = true;
          Emit("windowMinimized");
        }
      } else if (minimized_) {
        minimized_ = false;
        Emit("windowRestored");
      }
      break;
    default:
      break;
  }
}

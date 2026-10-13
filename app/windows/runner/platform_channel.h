#ifndef RUNNER_PLATFORM_CHANNEL_H_
#define RUNNER_PLATFORM_CHANNEL_H_

// The Windows side of the platform seams in app/lib/src/services/services.dart:
// clipboard hygiene (SEC-H01), capture exclusion (SEC-H02) and the OS events the
// auto-lock logic reacts to (SEC-A04). No security logic lives here: the timed clear,
// "only if still ours" and the lock decisions are Dart (ClipboardManager, AutoLockController).
// Nothing here touches the network.

#include <windows.h>

#include <flutter/binary_messenger.h>
#include <flutter/encodable_value.h>
#include <flutter/method_channel.h>
#include <flutter/method_result.h>

#include <memory>

class PlatformChannel {
 public:
  PlatformChannel(flutter::BinaryMessenger* messenger, HWND window);
  ~PlatformChannel();

  PlatformChannel(const PlatformChannel&) = delete;
  PlatformChannel& operator=(const PlatformChannel&) = delete;

  // Observes the window's messages (power, session lock, minimize) and forwards
  // them to Dart. Never consumes a message.
  void OnWindowMessage(UINT message, WPARAM wparam, LPARAM lparam);

 private:
  using Call = flutter::MethodCall<flutter::EncodableValue>;
  using Result = flutter::MethodResult<flutter::EncodableValue>;

  void Handle(const Call& call, std::unique_ptr<Result> result);
  void WriteClipboard(const Call& call, std::unique_ptr<Result> result);
  void Fingerprint(std::unique_ptr<Result> result);
  void ClearClipboard(std::unique_ptr<Result> result);
  void SetCaptureBlocked(const Call& call, std::unique_ptr<Result> result);
  void SaveFile(const Call& call, std::unique_ptr<Result> result);
  void PickFile(const Call& call, std::unique_ptr<Result> result);
  void Emit(const char* event);

  HWND window_;
  bool session_registered_ = false;
  bool minimized_ = false;
  std::unique_ptr<flutter::MethodChannel<flutter::EncodableValue>> channel_;
};

#endif  // RUNNER_PLATFORM_CHANNEL_H_

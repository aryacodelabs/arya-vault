import 'dart:async';

import 'package:arya_vault/src/services/auto_lock.dart';
import 'package:arya_vault/src/services/fake_services.dart';
import 'package:arya_vault/src/services/services.dart';
import 'package:flutter_test/flutter_test.dart';

// SEC-A04 (logic): lock on timeout, screen lock, sleep; reset on input.
void main() {
  late FakeTimeSource time;
  late List<AutoLockReason> locks;
  late AutoLockController controller;
  Future<void> Function(AutoLockReason) lockImpl = (_) async {};

  AutoLockController build({AutoLockConfig config = const AutoLockConfig()}) =>
      AutoLockController(
        time: time,
        config: config,
        lock: (AutoLockReason r) async {
          locks.add(r);
          await lockImpl(r);
        },
      );

  setUp(() {
    time = FakeTimeSource();
    locks = <AutoLockReason>[];
    lockImpl = (_) async {};
    controller = build();
  });

  tearDown(() => controller.dispose());

  Future<void> settle() => Future<void>.delayed(Duration.zero);

  group('timeout', () {
    test('locks after the configured idle time and not before', () async {
      controller.arm();
      time.advance(const Duration(minutes: 4, seconds: 59));
      await settle();
      expect(locks, isEmpty);
      time.advance(const Duration(seconds: 1));
      await settle();
      expect(locks, <AutoLockReason>[AutoLockReason.timeout]);
      expect(controller.armed, isFalse);
    });

    test('input resets the countdown', () async {
      controller.arm();
      time.advance(const Duration(minutes: 4));
      controller.activity();
      time.advance(const Duration(minutes: 4));
      await settle();
      expect(locks, isEmpty);
      time.advance(const Duration(minutes: 1, seconds: 1));
      await settle();
      expect(locks, hasLength(1));
    });

    test(
      'rapid input is cheap: one timer, never late by more than the rearm gap',
      () async {
        controller.arm();
        for (var i = 0; i < 500; i++) {
          time.advance(const Duration(milliseconds: 100));
          controller.activity();
        }
        expect(time.activeTimers, 1);
        expect(locks, isEmpty);
        // 50 s of input; the lock must come 5 min after the last input, give or
        // take the one-second rearm throttle (early, never late).
        time.advance(const Duration(minutes: 5));
        await settle();
        expect(locks, hasLength(1));
      },
    );

    test('does nothing while disarmed', () async {
      time.advance(const Duration(hours: 2));
      controller.activity();
      await controller.onPlatformEvent(PlatformEvent.sleep);
      expect(locks, isEmpty);
      expect(time.activeTimers, 0);
    });

    test('disarm cancels the pending lock', () async {
      controller.arm();
      controller.disarm();
      time.advance(const Duration(hours: 1));
      await settle();
      expect(locks, isEmpty);
    });

    test('arming twice keeps a single countdown', () async {
      controller.arm();
      controller.arm();
      expect(time.activeTimers, 1);
    });
  });

  group('settings change while armed', () {
    test('a shorter timeout counts from the last input', () async {
      controller.arm();
      time.advance(const Duration(minutes: 2));
      controller.configure(const AutoLockConfig(timeout: Duration(minutes: 3)));
      time.advance(const Duration(seconds: 59));
      await settle();
      expect(locks, isEmpty);
      time.advance(const Duration(seconds: 1));
      await settle();
      expect(locks, hasLength(1));
    });

    test('a timeout already exceeded locks at once', () async {
      controller.arm();
      time.advance(const Duration(minutes: 3));
      controller.configure(const AutoLockConfig(timeout: Duration(minutes: 1)));
      time.advance(Duration.zero);
      await settle();
      expect(locks, hasLength(1));
    });

    test('a longer timeout postpones the lock', () async {
      controller.arm();
      time.advance(const Duration(minutes: 4));
      controller.configure(
        const AutoLockConfig(timeout: Duration(minutes: 15)),
      );
      time.advance(const Duration(minutes: 10));
      await settle();
      expect(locks, isEmpty);
      time.advance(const Duration(minutes: 1, seconds: 1));
      await settle();
      expect(locks, hasLength(1));
    });

    test('turning the sleep option off stops sleep locking', () async {
      controller.arm();
      controller.configure(const AutoLockConfig(lockOnSleep: false));
      await controller.onPlatformEvent(PlatformEvent.sleep);
      expect(locks, isEmpty);
    });

    test('configure while disarmed arms nothing', () async {
      controller.configure(const AutoLockConfig(timeout: Duration(minutes: 1)));
      time.advance(const Duration(minutes: 10));
      await settle();
      expect(locks, isEmpty);
      expect(time.activeTimers, 0);
    });
  });

  group('platform events', () {
    test('sleep locks when enabled', () async {
      controller.arm();
      await controller.onPlatformEvent(PlatformEvent.sleep);
      expect(locks, <AutoLockReason>[AutoLockReason.sleep]);
    });

    test('screen lock locks when enabled', () async {
      controller.arm();
      await controller.onPlatformEvent(PlatformEvent.screenLocked);
      expect(locks, <AutoLockReason>[AutoLockReason.screenLocked]);
    });

    test('sleep and screen lock are ignored when disabled', () async {
      controller = build(
        config: const AutoLockConfig(
          lockOnSleep: false,
          lockOnScreenLock: false,
        ),
      )..arm();
      await controller.onPlatformEvent(PlatformEvent.sleep);
      await controller.onPlatformEvent(PlatformEvent.screenLocked);
      expect(locks, isEmpty);
      expect(controller.armed, isTrue);
    });

    test('minimize locks only when configured', () async {
      controller.arm();
      await controller.onPlatformEvent(PlatformEvent.windowMinimized);
      expect(locks, isEmpty);
      controller.configure(const AutoLockConfig(lockOnMinimize: true));
      await controller.onPlatformEvent(PlatformEvent.windowMinimized);
      expect(locks, <AutoLockReason>[AutoLockReason.windowMinimized]);
    });

    test(
      'resume after a long sleep with the sleep option off locks on the clock',
      () async {
        controller = build(config: const AutoLockConfig(lockOnSleep: false))
          ..arm();
        await controller.onPlatformEvent(PlatformEvent.sleep);
        time.jump(
          const Duration(hours: 3),
        ); // the machine slept; timers did not run
        await controller.onPlatformEvent(PlatformEvent.resume);
        expect(locks, <AutoLockReason>[AutoLockReason.timeout]);
      },
    );

    test('resume within the timeout keeps the vault unlocked', () async {
      controller = build(config: const AutoLockConfig(lockOnSleep: false))
        ..arm();
      time.advance(const Duration(minutes: 1));
      await controller.onPlatformEvent(PlatformEvent.resume);
      expect(locks, isEmpty);
      expect(controller.armed, isTrue);
      time.advance(const Duration(minutes: 4));
      await settle();
      expect(locks, hasLength(1));
    });
  });

  group('lock while locked and rapid events', () {
    test('events on a locked (disarmed) vault do nothing', () async {
      controller.arm();
      await controller.onPlatformEvent(PlatformEvent.sleep);
      locks.clear();
      await controller.onPlatformEvent(PlatformEvent.sleep);
      await controller.onPlatformEvent(PlatformEvent.screenLocked);
      time.advance(const Duration(hours: 1));
      await settle();
      expect(locks, isEmpty);
    });

    test('a burst of events locks once', () async {
      final Completer<void> gate = Completer<void>();
      lockImpl = (_) => gate.future;
      controller.arm();
      final Future<void> first = controller.onPlatformEvent(
        PlatformEvent.sleep,
      );
      final Future<void> second = controller.onPlatformEvent(
        PlatformEvent.screenLocked,
      );
      time.advance(const Duration(hours: 1)); // a timer firing meanwhile too
      gate.complete();
      await Future.wait(<Future<void>>[first, second]);
      expect(locks, hasLength(1));
    });

    test('a timer and an event at the same moment lock once', () async {
      controller.arm();
      time.advance(const Duration(minutes: 5));
      await controller.onPlatformEvent(PlatformEvent.sleep);
      await settle();
      expect(locks, hasLength(1));
    });

    test('re-arming after an unlock starts a fresh countdown', () async {
      controller.arm();
      await controller.onPlatformEvent(PlatformEvent.sleep);
      controller.arm();
      time.advance(const Duration(minutes: 4, seconds: 59));
      await settle();
      expect(locks, hasLength(1));
      time.advance(const Duration(seconds: 1));
      await settle();
      expect(locks, hasLength(2));
    });
  });

  group('failing lock', () {
    test('retries instead of staying unlocked', () async {
      var failures = 2;
      lockImpl = (_) async {
        if (failures-- > 0) throw StateError('io');
      };
      controller.arm();
      time.advance(const Duration(minutes: 5));
      await settle();
      expect(locks, hasLength(1));
      expect(controller.armed, isTrue);
      time.advance(const Duration(seconds: 5));
      await settle();
      expect(locks, hasLength(2));
      time.advance(const Duration(seconds: 5));
      await settle();
      expect(locks, hasLength(3));
      expect(controller.armed, isFalse);
    });
  });

  test('dispose stops everything', () async {
    controller.arm();
    controller.dispose();
    time.advance(const Duration(hours: 1));
    await settle();
    expect(locks, isEmpty);
    expect(time.activeTimers, 0);
  });
}

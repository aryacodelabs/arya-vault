package invalid.placeholder.arya_vault

import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine

class MainActivity : FlutterActivity() {
    private var platformChannel: PlatformChannel? = null

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        platformChannel = PlatformChannel(this, flutterEngine.dartExecutor.binaryMessenger)
    }

    override fun cleanUpFlutterEngine(flutterEngine: FlutterEngine) {
        platformChannel?.dispose()
        platformChannel = null
        super.cleanUpFlutterEngine(flutterEngine)
    }
}

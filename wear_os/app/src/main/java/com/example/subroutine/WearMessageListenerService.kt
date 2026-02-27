package com.example.subroutine

import android.content.Intent
import com.google.android.gms.wearable.MessageEvent
import com.google.android.gms.wearable.WearableListenerService

class WearMessageListenerService : WearableListenerService() {

    override fun onMessageReceived(event: MessageEvent) {
        if (event.path == "/pipeline/load/response") {
            val intent = Intent(ACTION_PIPELINE_RESPONSE).apply {
                putExtra(EXTRA_PAYLOAD, event.data)
                setPackage(packageName)
            }
            sendBroadcast(intent)
        }
    }

    companion object {
        const val ACTION_PIPELINE_RESPONSE =
            "com.example.subroutine.ACTION_PIPELINE_RESPONSE"
        const val EXTRA_PAYLOAD = "payload"
    }
}

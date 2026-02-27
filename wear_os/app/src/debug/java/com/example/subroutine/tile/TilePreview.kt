package com.example.subroutine.tile

import androidx.wear.tiles.RequestBuilders
import androidx.wear.tiles.tooling.preview.Preview
import androidx.wear.tiles.tooling.preview.TilePreviewData
import androidx.wear.tiles.tooling.preview.TilePreviewHelper

@Preview
fun tilePreview(): TilePreviewData {
    return TilePreviewData(
        onTileResourceRequest = { requestParams -> resources(requestParams) },
        onTileRequest = { requestParams ->
            TilePreviewHelper.singleTimelineEntryTileBuilder(
                tileLayout(requestParams, requestParams::class.java.let {
                    // tileLayout requires a Context; in a preview this is unavailable
                    // from TileRequest directly. We satisfy the compiler by supplying
                    // the layout element directly instead.
                    throw UnsupportedOperationException()
                })
            ).build()
        },
    )
}

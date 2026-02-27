package com.example.subroutine

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

sealed interface PipelineUiState {
    data object Loading : PipelineUiState
    data class Success(val pipeline: PipelinePayload) : PipelineUiState
    data class Error(val message: String) : PipelineUiState
}

class PipelineViewModel(application: Application) : AndroidViewModel(application) {

    private val repository = PipelineRepository(application)

    private val _uiState = MutableStateFlow<PipelineUiState>(PipelineUiState.Loading)
    val uiState: StateFlow<PipelineUiState> = _uiState

    init {
        loadPipeline()
    }

    fun loadPipeline() {
        viewModelScope.launch {
            _uiState.value = PipelineUiState.Loading
            _uiState.value = withContext(Dispatchers.IO) {
                runCatching { PipelineUiState.Success(repository.loadPipeline()) }
                    .getOrElse { PipelineUiState.Error(it.message ?: "Unknown error") }
            }
        }
    }

    fun instantiateSavedAction(savedActionId: String) {
        viewModelScope.launch {
            val result = withContext(Dispatchers.IO) {
                runCatching { repository.instantiateSavedAction(savedActionId) }
            }
            if (result.isSuccess) loadPipeline()
        }
    }

    fun promote(entryId: String) {
        viewModelScope.launch {
            val success = withContext(Dispatchers.IO) {
                runCatching { repository.promotePipelineEntry(entryId) }.getOrElse { false }
            }
            if (success) loadPipeline()
        }
    }

    fun demote(entryId: String) {
        viewModelScope.launch {
            val success = withContext(Dispatchers.IO) {
                runCatching { repository.demotePipelineEntry(entryId) }.getOrElse { false }
            }
            if (success) loadPipeline()
        }
    }

    fun deleteAction(actionId: String) {
        viewModelScope.launch {
            val success = withContext(Dispatchers.IO) {
                runCatching { repository.deletePipelineAction(actionId) }.getOrElse { false }
            }
            if (success) loadPipeline()
        }
    }
}

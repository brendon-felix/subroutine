package com.example.subroutine

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

sealed interface ActionsUiState {
    data object Loading : ActionsUiState
    data class Success(val actions: List<SavedAction>) : ActionsUiState
    data class Error(val message: String) : ActionsUiState
}

class ActionsViewModel(application: Application) : AndroidViewModel(application) {

    private val repository = ActionsRepository(application)

    private val _uiState = MutableStateFlow<ActionsUiState>(ActionsUiState.Loading)
    val uiState: StateFlow<ActionsUiState> = _uiState

    init {
        loadActions()
    }

    fun loadActions() {
        viewModelScope.launch {
            _uiState.value = ActionsUiState.Loading
            _uiState.value = withContext(Dispatchers.IO) {
                runCatching { ActionsUiState.Success(repository.fetchSavedActions()) }
                    .getOrElse { ActionsUiState.Error(it.message ?: "Unknown error") }
            }
        }
    }

    fun addAction(title: String, content: String?) {
        viewModelScope.launch {
            val success = withContext(Dispatchers.IO) {
                runCatching { repository.insertSavedAction(title, content) }
                    .getOrElse { false }
            }
            if (success) loadActions()
        }
    }

    fun deleteAction(id: String) {
        viewModelScope.launch {
            val success = withContext(Dispatchers.IO) {
                runCatching { repository.deleteSavedAction(id) }
                    .getOrElse { false }
            }
            if (success) loadActions()
        }
    }
}

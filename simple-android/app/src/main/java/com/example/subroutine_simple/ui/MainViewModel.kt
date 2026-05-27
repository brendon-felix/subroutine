package com.example.subroutine_simple.ui

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.example.subroutine_simple.data.models.Action
import com.example.subroutine_simple.data.models.Event
import com.example.subroutine_simple.data.models.QueueItem
import com.example.subroutine_simple.data.models.isBacklogged
import com.example.subroutine_simple.data.models.isNotFullyPassed
import com.example.subroutine_simple.data.models.isQueued
import com.example.subroutine_simple.data.repository.SubroutineRepository
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

sealed interface ActionsUiState {
    data object Loading : ActionsUiState
    data class Success(
        val queueItems: List<QueueItem>,
        val backlogged: List<Action>,
    ) : ActionsUiState
    data class Error(val message: String) : ActionsUiState
}

class MainViewModel : ViewModel() {

    private val repository = SubroutineRepository()

    private val _uiState = MutableStateFlow<ActionsUiState>(ActionsUiState.Loading)
    val uiState: StateFlow<ActionsUiState> = _uiState.asStateFlow()

    private val _completing = MutableStateFlow<Set<String>>(emptySet())
    val completing: StateFlow<Set<String>> = _completing.asStateFlow()

    private val _showCreateSheet = MutableStateFlow(false)
    val showCreateSheet: StateFlow<Boolean> = _showCreateSheet.asStateFlow()

    /** True while an edit/queue/backlog save is in-flight. */
    private val _saving = MutableStateFlow(false)
    val saving: StateFlow<Boolean> = _saving.asStateFlow()

    init {
        loadActions()
    }

    fun loadActions() {
        viewModelScope.launch {
            _uiState.value = ActionsUiState.Loading
            runCatching { repository.fetchAll() }
                .onSuccess { (actions, events) ->
                    val queuedActions = actions
                        .filter { it.isQueued }
                        .map { QueueItem.ActionItem(it) }
                    val relevantEvents = events
                        .filter { it.isNotFullyPassed }
                        .map { QueueItem.EventItem(it) }
                    val queueItems = (queuedActions + relevantEvents)
                        .sortedBy { it.sortKey }
                    _uiState.value = ActionsUiState.Success(
                        queueItems = queueItems,
                        backlogged = actions.filter { it.isBacklogged },
                    )
                }
                .onFailure { e ->
                    _uiState.value = ActionsUiState.Error(e.message ?: "Unknown error")
                }
        }
    }

    fun completeAction(id: String) {
        viewModelScope.launch {
            _completing.value = _completing.value + id
            runCatching { repository.completeAction(id) }
                .onSuccess { loadActions() }
                .onFailure { e ->
                    _completing.value = _completing.value - id
                    _uiState.value = ActionsUiState.Error(e.message ?: "Complete failed")
                }
            _completing.value = _completing.value - id
        }
    }

    fun openCreateSheet() {
        _showCreateSheet.value = true
    }

    fun closeCreateSheet() {
        _showCreateSheet.value = false
    }

    /**
     * Saves edits for [actionId]. The action is looked up from the current loaded state.
     */
    fun saveAction(actionId: String, title: String, content: String?, onSuccess: () -> Unit) {
        val action = resolveAction(_uiState.value, actionId) ?: return
        saveActionObject(action, title, content, onSuccess)
    }

    private fun saveActionObject(action: Action, title: String, content: String?, onSuccess: () -> Unit) {
        viewModelScope.launch {
            _saving.value = true
            runCatching { repository.updateAction(action, title, content) }
                .onSuccess {
                    loadActions()
                    onSuccess()
                }
                .onFailure { e ->
                    _uiState.value = ActionsUiState.Error(e.message ?: "Save failed")
                }
            _saving.value = false
        }
    }

    fun queueEditingAction(actionId: String, onSuccess: () -> Unit) {
        viewModelScope.launch {
            _saving.value = true
            runCatching { repository.queueAction(actionId) }
                .onSuccess {
                    loadActions()
                    onSuccess()
                }
                .onFailure { e ->
                    _uiState.value = ActionsUiState.Error(e.message ?: "Queue failed")
                }
            _saving.value = false
        }
    }

    fun backlogEditingAction(actionId: String, onSuccess: () -> Unit) {
        viewModelScope.launch {
            _saving.value = true
            runCatching { repository.backlogAction(actionId) }
                .onSuccess {
                    loadActions()
                    onSuccess()
                }
                .onFailure { e ->
                    _uiState.value = ActionsUiState.Error(e.message ?: "Backlog failed")
                }
            _saving.value = false
        }
    }

    fun createAction(title: String) {
        if (title.isBlank()) return
        viewModelScope.launch {
            runCatching { repository.createAction(title.trim()) }
                .onSuccess {
                    _showCreateSheet.value = false
                    loadActions()
                }
                .onFailure { e ->
                    _uiState.value = ActionsUiState.Error(e.message ?: "Create failed")
                }
        }
    }

    fun saveEvent(eventId: String, title: String, content: String?, onSuccess: () -> Unit) {
        val event = resolveEvent(_uiState.value, eventId) ?: return
        viewModelScope.launch {
            _saving.value = true
            runCatching { repository.updateEvent(event, title, content) }
                .onSuccess {
                    loadActions()
                    onSuccess()
                }
                .onFailure { e ->
                    _uiState.value = ActionsUiState.Error(e.message ?: "Save failed")
                }
            _saving.value = false
        }
    }

    fun deleteEvent(eventId: String, onSuccess: () -> Unit) {
        viewModelScope.launch {
            _saving.value = true
            runCatching { repository.deleteEvent(eventId) }
                .onSuccess {
                    loadActions()
                    onSuccess()
                }
                .onFailure { e ->
                    _uiState.value = ActionsUiState.Error(e.message ?: "Delete failed")
                }
            _saving.value = false
        }
    }

    private fun resolveEvent(state: ActionsUiState, eventId: String?): Event? {
        if (eventId == null) return null
        if (state !is ActionsUiState.Success) return null
        return state.queueItems
            .filterIsInstance<QueueItem.EventItem>()
            .map { it.event }
            .firstOrNull { it.id == eventId }
    }

    private fun resolveAction(state: ActionsUiState, actionId: String?): Action? {
        if (actionId == null) return null
        if (state !is ActionsUiState.Success) return null
        val fromQueue = state.queueItems
            .filterIsInstance<QueueItem.ActionItem>()
            .map { it.action }
            .firstOrNull { it.id == actionId }
        return fromQueue ?: state.backlogged.firstOrNull { it.id == actionId }
    }
}

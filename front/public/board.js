$(document).ready(async function() {

    if (theCookieExist('x-auth')) {

      const startTitleInput = document.getElementById('board-title');
      const startDescriptionInput = document.getElementById('board-description');

      startTitleInput.value = "";
      startDescriptionInput.value = "";


      var board_id = document.getElementsByClassName("message")[0].innerHTML;

      let token = getCookieValue('x-auth');

      let boards_data_request = await fetch('/user_boards', {
        method: 'GET',
        headers: {
            'Content-Type': 'application/json;charset=utf-8', 
            'Authorization': 'Bearer ' + token
        }

      });

      let boards_data_request_status = boards_data_request.status; 
      if (boards_data_request_status == 200) {

        let boards_data = await boards_data_request.json();

        boards_data.forEach(function(board) {

          if (board.id == board_id) {
            startTitleInput.value = board.title;
            startDescriptionInput.value = board.description;
          }
        });
      } else {
        alert("Can't fetch board's parameters.");
      }

      let tasks_data_request = await fetch('/board_tasks', {
          method: 'GET',
          headers: {
              'Content-Type': 'application/json;charset=utf-8', 
              'Authorization': 'Bearer ' + getCookieValue('x-auth'),
              'BoardId': board_id
          }

      });

      let request_result = tasks_data_request.status; 
      if (request_result == 200) {
          let tasks_data = await tasks_data_request.json();

          displayTasks(tasks_data, board_id);
      } else if (request_result == 401) {
        window.location.href = '/';
      } else {
          alert('Out of service. Please try later.');
      }

      const updateButton = document.getElementById('update-button');
      const deleteButton = document.getElementById('delete-button');

      updateButton.addEventListener('click', async function() {

        const updateTitleInput = document.getElementById('board-title');
        const updateDescriptionInput = document.getElementById('board-description');

        if (updateDescriptionInput.value.length > 200) {
          alert("Description couldn't be longer than 200 signs!")
        } else if (updateDescriptionInput.value.length == 0) {
          alert("Description couldn't be empty!")
        } else if (updateTitleInput.value.length == 0) {
          alert("Title couldn't be be empty!")
        } else if (updateTitleInput.value.length > 25) {
          alert("Title couldn't be longer than 25 signs!")
        } else {
          showOverlay();

          let board_update_body = {
              "id": parseInt(board_id, 10),
              "title": updateTitleInput.value,
              "description": updateDescriptionInput.value
          };
          let board_update_request = await fetch('/change_board', {
              method: 'PUT',
              headers: {
                  'Content-Type': 'application/json;charset=utf-8', 
                  'Authorization': 'Bearer ' + token,
              },
              body: JSON.stringify(board_update_body)
          });

          let board_update_request_status = board_update_request.status; 
          if (board_update_request_status == 200) {
              hideOverlay();
          } else {
              hideOverlay();
              alert('Unexpected issue happened. \nPlease try later.');  
          }

        }

      });

      deleteButton.addEventListener('click', async function() {

        const confirmed = confirm('Are you sure that you want to delete this board?');

          if (confirmed) {

          showOverlay();

          let board_delete_request = await fetch('/delete_board', {
            method: 'DELETE',
            headers: {
                'Content-Type': 'application/json;charset=utf-8', 
                'Authorization': 'Bearer ' + token
            },
            body: JSON.stringify({
                "id": parseInt(board_id, 10)
            })
          });

          let board_delete_request_status = board_delete_request.status; 
          if (board_delete_request_status == 200) {
            hideOverlay();
            window.location.replace("/boards");
          } else {
            hideOverlay();
            alert('Unexpected issue happened. \nPlease try later.');  
          }
        }

      });

      const createTaskButton = document.getElementById('createTaskButton');
      const createTaskModal = document.getElementById('createTaskModal');
      const closeCreateTaskModal = createTaskModal.querySelector('.close');

      const textarea = document.getElementById('task-description');

      textarea.addEventListener('keydown', async function(e) {
      if (e.key === 'Tab') {
          e.preventDefault(); 
          const start = this.selectionStart;
          const end = this.selectionEnd;

          this.value = this.value.substring(0, start) + '\t ' + this.value.substring(end);
          
          this.selectionStart = this.selectionEnd = start + 1;
      }
      });

      createTaskButton.addEventListener('click', () => {
        createTaskModal.style.display = 'block';
      });

      closeCreateTaskModal.addEventListener('click', () => {
        createTaskModal.style.display = 'none';
        document.getElementById('task-title').value = '';
        document.getElementById('task-description').value = '';
      });

      const createTaskForm = document.getElementById('createTaskForm');

      createTaskForm.addEventListener('submit', async function(event) {
        event.preventDefault();

        const title = document.getElementById('task-title').value;
        const description = document.getElementById('task-description').value;

        if (title.length > 50) {
          alert("Name couldn't be longer than 50 signs")
        } else if (description.length > 2000) {
          alert("Description couldn't be longer than 2000 signs")
        } else {
          
          showOverlay();

          let new_task_data = {
            "board_id": parseInt(board_id, 10),
            "title": title,
            "description": description
          };
    
          let new_task_request = await fetch('/create_task', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json;charset=utf-8', 
                'Authorization': 'Bearer ' + token
            },
            body: JSON.stringify(new_task_data)
    
          });
          let new_task_request_status = new_task_request.status; 
    
          if (new_task_request_status == 200) {
            hideOverlay();
            window.location.replace(`/board/${board_id}`);
          } else {
            hideOverlay();
            createTaskModal.style.display = 'none';
            document.getElementById('task-title').value = '';
            document.getElementById('task-description').value = '';
            alert('Unexpected issue happened. \nPlease try later.');  
          }
        }
      });

    } else {
      window.location.href = '/';
    }
});
  
  function displayTasks(tasks, boardId) {
    var listsContainer = $('#listsContainer');
  
    listsContainer.empty();
  
    var tasksByStatus = groupTasksByStatus(tasks);
    var statusMap = {
        0: "To do", 
        1: "In progress",
        2: "Done",
        3: "On hold"
    };
  
    for (var status in statusMap) {

      var columnClass = `status${status}`;
      var list = $(`<div class="column ${columnClass}"><a class="status-name">${statusMap[status]}</a></div>`);
      
      var tasksList = $('<ul></ul>');

      if (status in tasksByStatus) {
        tasksByStatus[status].forEach(function(task) {

            var taskURL = `/show_task/${boardId}/${task.id}`;
            var taskItem = $(`<a class="task-text" href="${taskURL}"></a>`);
            var taskTitle = $('<p class="task">' + task.title + '</p>'); 

            taskItem.append(taskTitle);

            tasksList.append(taskItem);
          });
        list.append(tasksList);
      }

      listsContainer.append(list);
    }
  }


function groupTasksByStatus(tasks) {
    var tasksByStatus = {};
  
    tasks.forEach(function(task) {
      var status = task.status_id;
  
      if (!tasksByStatus[status]) {
        tasksByStatus[status] = [];
      }
  
      tasksByStatus[status].push(task);
    });
  
    return tasksByStatus;
  }
  

  function theCookieExist(cookieName) {
    var cookies = document.cookie.split(';');
  
    for (var i = 0; i < cookies.length; i++) {
      var cookie = cookies[i].trim();
  
      if (cookie.startsWith(cookieName + '=')) {
        return true;
      }
    }
  
    return false;
}

function getCookieValue(cookieName) {
    const cookie = document.cookie.match('(^|;)\\s*' + cookieName + '\\s*=\\s*([^;]+)');
    return cookie ? cookie.pop() : '';
}

function showOverlay() {
    document.getElementById("overlay").style.display = "flex";
}
    
function hideOverlay() {
    document.getElementById("overlay").style.display = "none";
}
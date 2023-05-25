$(document).ready(async function() {

    if (theCookieExist('x-auth')) {

        const titleElement = document.getElementById('title');
        const descriptionElement = document.getElementById('description');
        const statusElement = document.getElementById('status');
        const saveButton = document.getElementById('saveButton');
        const deleteButton = document.getElementById('deleteButton');

        saveButton.disabled = true;
        const taskContent = document.getElementById('taskContent');

        var board_id = document.getElementsByClassName("message-board")[0].innerHTML;
        var task_id = document.getElementsByClassName("message-task")[0].innerHTML;

        let newURL = `/board/${board_id}`;
        $("#board-link").attr("href", newURL);


        let token = getCookieValue('x-auth');
        var task_request_result = await fetch(`/task/${task_id}`, {
            method: 'GET',
            headers: {
                'Content-Type': 'application/json;charset=utf-8', 
                'Authorization': 'Bearer ' + token,
                'BoardId': board_id
            }
        });

        

        let request_result = task_request_result.status; 
        let taskData = {};
        if (request_result == 200) {
            taskData = await task_request_result.json();

            var creationTime = taskData['creation_time']

            var formattedTime = formatTime(creationTime);

            $("#taskCreationTime").text("Created at: " + formattedTime);

            let title = taskData['title'];
            $("#title").text(title);

            let description = taskData['description'];
            $("#description").text(description);

            let status = taskData['status_id'];

            let set_01 = ''
            let set_02 = ''
            let set_03 = ''
            let set_04 = ''

            if (status == 0){
                set_01 = 'selected="selected"'
            } else if (status == 1) {
                set_02 = 'selected="selected"'
            } else if (status == 2) {
                set_03 = 'selected="selected"'
            } else if (status == 3) {
                set_04 = 'selected="selected"'
            };

            jQuery("#status").append(`<option value="0" ${set_01}>To Do</option>`);
            jQuery("#status").append(`<option value="1" ${set_02}>In Progress</option>`);
            jQuery("#status").append(`<option value="2" ${set_03}>Done</option>`);
            jQuery("#status").append(`<option value="3" ${set_04}>On Hold</option>`);

        } else if (request_result == 401) {
            window.location.href = '/';
        } else {
            alert('Out of service. Please try later.');
        }

        let statusMap = {
            0: "To Do", 
            1: "In Progress", 
            2: "Done", 
            3: "On Hold"
        };
        let originalTitle = taskData['title'];
        let originalDescription = taskData['description'];
        let taskStatus = taskData['status_id'];
        let originalStatus = statusMap[taskStatus];
        console.log(originalStatus);

        const textarea = document.getElementById('description');

        textarea.addEventListener('keydown', async function(e) {
        if (e.key === 'Tab') {
            e.preventDefault(); 
            const start = this.selectionStart;
            const end = this.selectionEnd;

            this.value = this.value.substring(0, start) + '\t ' + this.value.substring(end);
            
            this.selectionStart = this.selectionEnd = start + 1;
        }
        });

        titleElement.addEventListener('input', updateSaveButtonStatus);
        descriptionElement.addEventListener('input', updateSaveButtonStatus);
        statusElement.addEventListener('input', updateSaveButtonStatus);

        saveButton.addEventListener('click', async function() {


            if (descriptionElement.value.length > 2000) {
                alert("Description couldn't be longer than 2000 signs!")
            } else if (descriptionElement.value.length == 0) {
                alert("Description couldn't be empty!")
            } else if (titleElement.textContent.length == 0) {
                alert("Title couldn't be be empty!")
            } else if (titleElement.textContent.length > 50) {
                alert("Title couldn't be longer than 50 signs!")
            } else {

                showOverlay();
                let task_update_body = {
                    "id": parseInt(task_id, 10),
                    "board_id": parseInt(board_id, 10),
                    "title": titleElement.textContent, 
                    "description": descriptionElement.value,
                    "status_id": parseInt(statusElement.value, 10)
                }

                let task_update_request = await fetch('/change_task', {
                    method: 'PUT',
                    headers: {
                        'Content-Type': 'application/json;charset=utf-8', 
                        'Authorization': 'Bearer ' + token,
                    },
                    body: JSON.stringify(task_update_body)
                });
        
                let task_update_request_status = task_update_request.status; 
                console.log(task_update_request_status);
                if (task_update_request_status == 200) {
                    hideOverlay();

                    originalTitle = titleElement.textContent;
                    originalDescription = descriptionElement.textContent;
                    originalStatus = statusMap[parseInt(statusElement.value, 10)];

                    saveButton.disabled = true;

                } else {
                    hideOverlay();
                    alert('Unexpected issue happened. \nPlease try later.');  
                }


                console.log(originalStatus);

                saveButton.disabled = true;
            }
        });

        function updateSaveButtonStatus() {
            if (
            titleElement.textContent !== originalTitle ||
            descriptionElement.textContent !== originalDescription || 
            statusElement.textContent !== originalStatus
            ) {
            saveButton.disabled = false;
            } else {
            saveButton.disabled = true;
            }
        }

        deleteButton.addEventListener('click', async function() {

            const confirmed = confirm('Are you sure that you want to delete this task?');

            if (confirmed) {

                showOverlay();

                let task_delete_request = await fetch('/delete_task', {
                    method: 'DELETE',
                    headers: {
                        'Content-Type': 'application/json;charset=utf-8', 
                        'Authorization': 'Bearer ' + token
                    },
                    body: JSON.stringify({
                        "id": parseInt(task_id, 10), 
                        "board_id": parseInt(board_id, 10)
                    })
                });
        
                let task_delete_request_status = task_delete_request.status; 
                if (task_delete_request_status == 200) {
                    hideOverlay();
                    window.location.replace(newURL);
                } else {
                    hideOverlay();
                    alert('Unexpected issue happened. \nPlease try later.');  
                }
            
            }

        })

    } else {
        window.location.href = '/';
    }

  });


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

function formatTime(creationTime) {
    var time = new Date(creationTime * 1000); 
    var options = { year: 'numeric', month: 'long', day: 'numeric', hour: 'numeric', minute: 'numeric' };
    return time.toLocaleString('en-En', options);
}


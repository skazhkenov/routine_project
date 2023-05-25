$(document).ready(async function() {

  if (theCookieExist('x-auth')) {

    const startTitleInput = document.getElementById('board-title');
    const startDescriptionInput = document.getElementById('board-description');

    startTitleInput.value = "";
    startDescriptionInput.value = "";

    let token = getCookieValue('x-auth');
    let boards_data_request = await fetch('/user_boards', {
      method: 'GET',
      headers: {
          'Content-Type': 'application/json;charset=utf-8', 
          'Authorization': 'Bearer ' + token
      }

    });

    let request_result = boards_data_request.status; 
    if (request_result == 200) {
      var boardsList = $('#boardsList');

      let boards_data = await boards_data_request.json();

      boards_data.forEach(function(board) {

        var boardLink = "/board/SPECIAL".replace('SPECIAL', board.id);
        console.log(boardLink);

        var boardElement = `
          <a class="board" href="${boardLink}">
            <div class="board-text" >
              <h3>${board.title}</h3>
              <p>${board.description}</p>
            </div>
          </a>
        `;

        boardsList.append(boardElement);
      });
    } else if (request_result == 401) {
      window.location.href = '/';
    } else {
        alert('Out of service. Please try later.');
    }


    const createBoardForm = document.getElementById('create-board-form');

    createBoardForm.addEventListener('submit', async function(e) {
      e.preventDefault();

      const boardTitleInput = document.getElementById('board-title');
      const boardDescriptionInput = document.getElementById('board-description');

      const boardTitle = boardTitleInput.value;
      const boardDescription = boardDescriptionInput.value;

      if (boardDescription.length > 200) {
        alert("Description couldn't be longer than 200 signs!")
      } else if (boardTitle.length > 25) {
        alert("Title couldn't be longer than 25 signs!")
      } else {

        showOverlay();
        let new_board_data = {
          "title": boardTitle,
          "description": boardDescription
        };

        let new_board_request = await fetch('/create_board', {
          method: 'POST',
          headers: {
              'Content-Type': 'application/json;charset=utf-8', 
              'Authorization': 'Bearer ' + token
          },
          body: JSON.stringify(new_board_data)

        });
        let new_board_request_status = new_board_request.status; 

        if (new_board_request_status == 200) {
          hideOverlay();
          window.location.replace("/boards");  
        } else {
          hideOverlay();
          boardTitleInput.value = '';
          boardDescriptionInput.value = '';
          alert('Unexpected issue happened. \nPlease try later.');  
        }
      }

    });

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
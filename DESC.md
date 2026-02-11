# pizza-freenet

Pizza ordering helper application for freenet

# Design

State structure:
- name: string, contains name of order, e.g. "pizza for party"
- created_at: timestamp, when the order got created
- creator: id of user creating order
- items: key-value structure
  - key: id of user creating sub-order
  - value:
    - display_name: string, nickname of orderer
    - order: text description of order, e.g. 2x margherita
    - price: price of order
    - paid: boolean, default false. can only be updated by creator

Updates:
- initial contract update
  - create_order { name, creator, created_at } -> sets initial parameters
- creator operations
  - update_name { name } -> updates name
  - update_paid { sub_user, paid = false/true } -> set paid state for items.${sub_user}
- sub-user operations
  - add_item { display_name, order, price } -> add entry to items
  - edit_item { display_name?, order?, price? } -> edit entry in items
  - delete_item { } -> delete entryy in items

# Frontend

- Side-bar with already created and subscribed pizza-contracts
- Button "Add Order" -> create new pizza contract
  - Creates Dialog with:
    - Textbox, name: Order name
    - Button "Create", runs create_order operation
- View for existing pizza contracts
  - List of all items, showing display_name and order and paid and price in a table
    - Use checkmark for paid
    - Paid can be clicked if the user signed in is the contract creator, runs update_paid operation


